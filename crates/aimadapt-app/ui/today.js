// Today: the home page. Today's program workout with each drill's runs and rating,
// the Last session card (lastSessionCard, in session.js), the week at a glance, your favorites and
// what you played last (each with its play button), and the top note from your history.
// Loaded after program.js and scenarios.js and draws with their helpers.

function today() {
  const wait = waitFor([programNeed(), trainerNeed(), overviewNeed()], "Loading today…");
  if (wait) return wait;
  const s = ui.snap;
  const t = s.training;
  const date = day(t.today);
  const stat = t.days.find((d) => d.day === t.today);
  const goal = s.time_settings.daily_goal_min;
  const active = stat ? stat.active_s : 0;
  const streak = t.streak.current;
  return `<div class="page">
    ${head(esc(date.toLocaleDateString("en-US", { weekday: "long", month: "long", day: "numeric" })), "Today", `<span class="chip-link static">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--rust)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 3c1 3 4 5 4 9a4 4 0 0 1-8 0c0-2 1-3 2-4 0 2 1 3 2 3 0-3-1-5 0-8z"></path></svg>
      <strong>${streak} day${streak === 1 ? "" : "s"}</strong> streak · ${dur(active)} of ${goal} min today</span>`)}
    ${trainerBanners()}
    <div class="row" style="align-items:flex-start">
      <div style="flex:3 1 520px;min-width:0;display:flex;flex-direction:column;gap:16px">${todayWorkout()}${lastSessionCard()}</div>
      <div style="flex:2 1 300px;min-width:0;display:flex;flex-direction:column;gap:16px">${weekStrip()}${favoritesCard()}${recentlyPlayed()}</div>
    </div>
    ${worthKnowing()}
  </div>`;
}

function todayWorkout() {
  const d = ui.program.data;
  const { today, next, stat } = todayInfo();
  if (!today.workout) {
    return `<section class="card today-card"><div class="eyebrow">${esc(d.name)} · ${esc(today.name)}</div><h2>Rest day</h2>
      <div class="note-body">Next up: ${esc(next.name)}, ${esc(next.focus)}. If you want to play anyway, <button type="button" class="link-btn inline" data-action="list-tab-open" data-tab="browse">pick a scenario</button>.</div></section>`;
  }
  const drill = drillLookup();
  const share = stat && stat.planned ? Math.min(1, stat.done / stat.planned) : 0;
  return `<section class="card today-card">
    <div class="card-head" style="align-items:flex-start">
      <div><div class="eyebrow accent">${esc(d.name)} · ${esc(today.name)}</div><h2 class="detail-title">${esc(today.focus)}</h2>
        <div class="small muted">${today.workout.items.length} drills · ${today.runs} runs · about ${Math.round(today.minutes)} min · <a href="#settings/training" title="How adaptive runs mix target difficulty. Change it in Settings.">${esc(mixName(ui.trainer.data.training.mix))} mix</a></div></div>
      <span class="chips"><button class="btn" type="button" data-action="list-tab-open" data-tab="browse">Something else</button>
        ${sessionButton(today.day, stat)}</span>
    </div>
    ${stat && stat.planned ? `<div class="day-progress"><div class="small muted"><span>${stat.done} of ${stat.planned} runs</span><span>${stat.minutes ? `${Math.round(stat.minutes)} min in` : ""}</span></div>
      <div class="bar-track"><div class="bar-fill" style="width:${(share * 100).toFixed(0)}%"></div></div></div>` : ""}
    <div class="drills" role="table" aria-label="Today's drills">
      <div class="drill head" role="row"><span role="columnheader">Block</span><span role="columnheader">Drill</span><span role="columnheader">Runs</span><span role="columnheader" class="r">85% rating</span></div>
      ${today.workout.items.map((it) => {
        const { entry: e, name } = drill(it);
        const rating = it.mode !== "standard" && e && e.rating !== null && e.rating !== undefined ? fmtKnob(e.knob, e.rating) : "–";
        return `<div class="drill" role="row"><span role="cell" class="small muted">${esc(BLOCK_LABELS[it.block ?? "none"] || "")}</span>
          <span role="cell">${e?.scenario ? `<a href="${scenarioHref(e.id)}">${esc(name)}</a>` : esc(name)}</span>
          <span role="cell" class="muted">×${it.runs}${it.mode === "standard" ? " · standard" : ""}</span>
          <span role="cell" class="r num">${esc(rating)}</span></div>`;
      }).join("")}
    </div>
    <div class="small muted">A drill's name opens it on Scenarios, with its rating history, insights and replays.</div>
  </section>`;
}

function weekStrip() {
  const d = ui.program.data;
  const p = d.progress;
  return `<section class="card"><div class="card-head"><h2>This week</h2><a class="small" href="#scenarios/program">Whole week</a></div>
    <div class="week-strip">${p.week.map((w) => {
      const info = d.days.find((x) => x.day === w.day);
      const label = w.planned ? (w.minutes ? `${Math.round(w.minutes)}` : w.status === "missed" ? "missed" : "–") : "rest";
      return `<div class="ws-day ${w.status} ${w.day === d.today ? "is-today" : ""}" title="${esc(`${info.name}: ${info.focus ?? "Rest"}${w.planned ? ` · ${w.done} of ${w.planned} runs` : ""}`)}">
        <span class="ws-name">${esc(info.name.slice(0, 3))}</span><span class="ws-cell">${esc(label)}</span></div>`;
    }).join("")}</div>
    <div class="small muted">Minutes per day in ${esc(d.name)}.</div>
  </section>`;
}

// A row that opens the item, with its play button (playButton, on Scenarios).
function playRow(x, sub) {
  const play = playButton(x);
  return `<div class="list-row"><a class="main" href="${scenarioHref(x.key)}" style="color:inherit">
      <div class="title">${esc(x.name)}</div><div class="sub">${esc(sub)}</div></a>
    <div class="num">${esc(x.figure)}</div>${play}</div>`;
}

// Starred items, in the order they were starred.
function favoritesCard() {
  const order = marks().favorites;
  const favs = scenarioItems().filter((x) => isFavorite(x.key) && !isHidden(x.key)).sort((a, b) => order.indexOf(a.key) - order.indexOf(b.key));
  if (!favs.length) return "";
  return `<section class="card"><div class="card-head"><h2>Favorites</h2><button type="button" class="link-btn" data-action="list-tab-open" data-tab="browse">Browse</button></div>
    <div>${favs.slice(0, 6).map((x) => playRow(x, [x.kind === "workout" ? "Workout" : x.skill || "KovaaK's", x.last ? dayLabel(x.last) : "Not played yet"].join(" · "))).join("")}</div>
  </section>`;
}

function recentlyPlayed() {
  const recent = scenarioItems().filter((x) => x.last && !isHidden(x.key)).slice(0, 5);
  if (!recent.length) return "";
  return `<section class="card"><div class="card-head"><h2>Recently played</h2><button type="button" class="link-btn" data-action="list-tab-open" data-tab="browse" data-played="yes">All recent</button></div>
    <div>${recent.map((x) => playRow(x, `${dayLabel(x.last)}, ${fmtTime(dt(x.last))}${x.kind === "kovaaks" ? " · KovaaK's" : ""}`)).join("")}</div>
  </section>`;
}

// The top note from the ranked list (rankedNotes, in program.js). The whole list is on
// Progress › Skills.
function worthKnowing() {
  const [note] = rankedNotes();
  if (!note) return "";
  return `<section class="card"><div class="card-head"><h2>Worth knowing</h2><a class="small" href="#progress/skills">All notes</a></div>${noteView(note)}</section>`;
}
