// Session summary: what the last session in the trainer changed, on its own page
// (#session) and as the Last session card on Today. The data comes from the
// last_session command (crates/aimadapt-core/src/programs/recap.rs).
// Loaded after today.js and draws with the helpers of program.js and scenarios.js.

// data: undefined until loaded, then the recap or null (no trainer runs yet).
// from: the page the player was on when the trainer closed, for the Back link.
ui.session = { data: undefined, error: null, from: null };

async function loadSession(force = false) {
  if (ui.session.data !== undefined && !force) return;
  try {
    ui.session.data = (await invoke("last_session")) ?? null;
    ui.session.error = null;
  } catch (e) {
    ui.session.error = String(e);
  }
  if (["today", "session"].includes(route().view)) render();
}

const sessionNeed = () => ({ key: "session", have: ui.session.data !== undefined, error: ui.session.error, load: loadSession });

// An import can add KovaaK's runs to the session's count.
listen("runs-imported", () => {
  if (ui.session.data !== undefined) loadSession(true);
});

// ---------- the page ----------

function sessionHead(r) {
  const eyebrow = r
    ? `${esc(day(r.day).toLocaleDateString("en-US", { weekday: "long", month: "long", day: "numeric" }))} · ${esc(fmtTime(dt(r.start)))} to ${esc(fmtTime(dt(r.end)))}`
    : "";
  // Back goes to the page the trainer was started from. Today has its own button.
  const from = ui.session.from;
  const back = from && !["#session", "#today"].includes(from) ? `<a class="btn" href="${esc(from)}">Back</a>` : "";
  return head(eyebrow, "Session summary", `${back}<a class="btn" href="#today">Today</a>`);
}

function sessionTiles(r) {
  const more = r.runs > r.trainer_runs ? ` · ${r.runs - r.trainer_runs} more in KovaaK's` : "";
  return `<section class="kpis" aria-label="Session totals">
    ${kpi("Active time", dur(r.active_s), `${dur(r.length_s)} from first run to last`)}
    ${kpi("Runs", r.trainer_runs, `${plural(r.scenarios.length, "scenario")}${more}`)}
    ${kpi("Breakthroughs", r.breakthroughs, r.breakthroughs ? "Harder levels held than ever before" : "None this session")}
  </section>`;
}

const muted = (text) => `<div class="small muted">${text}</div>`;

// The rating column: before and after, or the first rating, or the best standard score.
function sessionRating(s) {
  if (s.before !== null && s.after !== null) return `${esc(fmtKnob(s.knob, s.before))} → ${esc(fmtKnob(s.knob, s.after))}`;
  if (s.after !== null) return `${esc(fmtKnob(s.knob, s.after))}${muted("First rating on this version")}`;
  if (s.best_score !== null) return `Best score ${esc(num(s.best_score))}${muted("Standard runs")}`;
  return "–";
}

const noWrap = (html) => `<span style="white-space:nowrap">${html}</span>`;

// The change column: the change, what it is measured from, and the program day's change.
// Only the numbers stay on one line. The program day's line is long, and kept on one line
// it pushed the table past its card at the window's smallest width.
function sessionChange(s) {
  const since = s.before_date ? muted(`since ${esc(fmtShort(day(s.before_date)))}`) : "";
  const week = s.week
    ? muted(`${noWrap(changeSpan(s.week.change, s.week.spread, 1))} since the same program day on ${esc(fmtShort(day(s.week.date)))}`)
    : "";
  return `${noWrap(changeSpan(s.change, s.spread, 1))}${since}${week}`;
}

function sessionRow(s) {
  return `<tr><td><a href="${scenarioHrefByName(s.scenario)}">${esc(s.scenario)}</a></td>
    <td class="r mono">${s.runs}</td>
    <td class="r mono" style="white-space:nowrap">${sessionRating(s)}</td>
    <td class="r mono">${sessionChange(s)}</td>
    <td class="r mono">${s.breakthroughs || "–"}</td></tr>`;
}

function sessionScenarios(r) {
  return `<section class="card"><div class="card-head"><h2>Scenarios</h2></div>
    <div class="table-wrap"><table><thead><tr><th>Scenario</th><th class="r">Runs</th><th class="r">85% rating</th><th class="r">Change</th><th class="r">Breakthroughs</th></tr></thead>
      <tbody>${r.scenarios.map(sessionRow).join("")}</tbody></table></div>
    <div class="small muted">A change is grey when zero is inside its likely range. One session moves a rating a little, so look at the trend on the scenario's page.</div>
  </section>`;
}

function sessionNote(r) {
  if (!r.note) return "";
  return `<section class="card"><div class="card-head"><h2>Worth knowing</h2><a class="small" href="${scenarioHref(`name:${r.note.scenario}`, "insights")}">${esc(r.note.scenario)}</a></div>${noteView({ title: r.note.title, body: r.note.body })}</section>`;
}

function sessionPage() {
  const wait = waitFor([sessionNeed()], "Loading the session…");
  if (wait) return wait;
  const r = ui.session.data;
  if (!r) {
    return `<div class="page">${sessionHead(null)}
      <div class="empty-state">No sessions in the trainer yet. Play a scenario and its summary shows here.</div></div>`;
  }
  return `<div class="page">${sessionHead(r)}${sessionTiles(r)}${sessionScenarios(r)}${sessionNote(r)}</div>`;
}

// ---------- the card on Today ----------

// The line under a row's name: the rating before and after, or what else there is.
function lastSessionSub(s) {
  if (s.before !== null && s.after !== null) return `${fmtKnob(s.knob, s.before)} → ${fmtKnob(s.knob, s.after)}`;
  if (s.after !== null) return `First rating: ${fmtKnob(s.knob, s.after)}`;
  if (s.best_score !== null) return `Best score ${num(s.best_score)}`;
  return plural(s.runs, "run");
}

function lastSessionRow(s) {
  const figure = s.change === null ? "" : changeSpan(s.change, s.spread, 1);
  return `<div class="list-row"><a class="main" href="${scenarioHrefByName(s.scenario)}" style="color:inherit">
      <div class="title">${esc(s.scenario)}</div><div class="sub">${esc(lastSessionSub(s))}</div></a>
    <div class="num">${figure}</div></div>`;
}

// Returns "" until the recap is in and when there is none. Today does not wait for it.
function lastSessionCard() {
  if (ui.session.data === undefined && !ui.session.error) startOnce("session", loadSession);
  const r = ui.session.data;
  if (!r) return "";
  const shown = r.scenarios.slice(0, 6);
  const extra = r.scenarios.length - shown.length;
  const broke = r.breakthroughs ? ` · ${plural(r.breakthroughs, "breakthrough")}` : "";
  return `<section class="card"><div class="card-head"><h2>Last session</h2><a class="small" href="#session">Summary</a></div>
    <div class="small muted">${esc(dayLabel(r.start))}, ${esc(fmtTime(dt(r.start)))} · ${esc(dur(r.active_s))} active · ${plural(r.trainer_runs, "run")}${broke}</div>
    <div>${shown.map(lastSessionRow).join("")}</div>
    ${extra > 0 ? `<a class="small" href="#session">${extra} more in the summary</a>` : ""}
  </section>`;
}
