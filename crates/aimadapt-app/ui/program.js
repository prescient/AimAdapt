// The weekly training plan (Core Week): today's workout (on Today), and the week, the
// skill profile and per-drill session gains (Progress › Skills). Loaded after
// trainer.js. Data comes from the `program` command (crates/aimadapt-core/src/programs.rs).

ui.program = { data: null, error: null };

const SKILL_LABELS = {
  static_clicking: "Static clicking",
  dynamic_clicking: "Dynamic clicking",
  linear_clicking: "Linear clicking",
  precise_tracking: "Precise tracking",
  reactive_tracking: "Reactive tracking",
  control_tracking: "Control tracking",
  speed_switching: "Speed switching",
  evasive_switching: "Evasive switching",
  stability_switching: "Stability switching",
};
const BLOCK_LABELS = { warmup: "Warm-up", main: "Main", secondary: "Secondary", maintenance: "Maintenance", none: "" };
const STATUS_LABELS = { done: "Done", partial: "Partial", missed: "Missed", today: "Today", upcoming: "", rest: "Rest" };

const pctChange = (v, digits = 0) => (v === null || v === undefined ? "–" : `${v >= 0 ? "+" : "−"}${Math.abs(v * 100).toFixed(digits)}%`);

async function loadProgram(force = false) {
  if (ui.program.data && !force) return;
  try {
    ui.program.data = await invoke("program");
    ui.program.error = null;
  } catch (e) {
    ui.program.error = String(e);
  }
  if (["today", "progress"].includes(route().view)) render();
}

// What views that need the program pass to waitFor (app.js).
const programNeed = () => ({ key: "program", have: Boolean(ui.program.data), error: ui.program.error, load: loadProgram });

function todayInfo() {
  const d = ui.program.data;
  if (!d) return null;
  const today = d.days.find((x) => x.day === d.today);
  const order = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
  const i = order.indexOf(d.today);
  const next = [1, 2, 3, 4, 5, 6, 7].map((k) => d.days.find((x) => x.day === order[(i + k) % 7])).find((x) => x.workout);
  const stat = d.progress.week.find((w) => w.day === d.today);
  return { today, next, stat };
}

// Progress › Skills: how each sub-skill and the week are going.
function skillsTab() {
  const st = ui.program;
  if (st.error) return `<div class="card error">${esc(st.error)}</div>`;
  const wait = waitFor([programNeed(), trainerNeed()], "Loading…");
  if (wait) return wait;
  const d = st.data;
  const p = d.progress;
  const s = p.summary;
  return `${trainerErrorBanner()}
    <section class="kpis" aria-label="This week in numbers">
      ${kpi("Days done", `${s.days_done} <small>of ${s.days_planned}</small>`, `${esc(d.name)} this week`)}
      ${kpi("Minutes", Math.round(s.minutes), "This week, in the program")}
      ${s.best ? kpi("Biggest gain", esc(SKILL_LABELS[s.best[0]]), `${pctChange(s.best[1], 1)} over the last 7 days`) : ""}
    </section>
    <div class="row" style="align-items:flex-start">
      ${profileCard(p.profile)}
      ${summaryCard(d)}
    </div>
    ${weekCard(d)}
    ${sessionsCard(p.sessions)}`;
}

function weekCard(d) {
  const p = d.progress;
  return `<section class="card"><div class="card-head"><h2>This week</h2><span class="small muted">Week of ${esc(fmtShort(day(p.week_start)))}</span></div>
    <div class="week-grid">${p.week.map((w) => {
      const info = d.days.find((x) => x.day === w.day);
      const share = w.planned ? Math.min(1, w.done / w.planned) : 0;
      return `<div class="week-day ${w.status} ${w.day === d.today ? "is-today" : ""}">
        <div class="wd-name">${esc(info.name.slice(0, 3))} <span class="small muted">${esc(fmtShort(day(w.date)))}</span></div>
        <div class="small muted wd-focus">${esc(info.focus ?? "Rest")}</div>
        ${w.planned ? `<div class="bar-track"><div class="bar-fill" style="width:${share * 100}%"></div></div>
          <div class="small">${w.done} / ${w.planned} runs${w.minutes ? ` · ${Math.round(w.minutes)} min` : ""}</div>` : `<div class="small muted">Rest day</div>`}
        ${STATUS_LABELS[w.status] ? `<span class="tag ${w.status === "done" ? "" : "grey"}">${STATUS_LABELS[w.status]}</span>` : ""}
        ${w.focus_change !== null ? `<div class="small"><span class="${w.focus_change >= 0 ? "pos" : "neg"}">${pctChange(w.focus_change, 1)}</span> vs last ${esc(info.name)}</div>` : ""}
        ${w.planned && w.status !== "done" ? `<button class="btn small-btn" type="button" data-action="play-program" data-day="${w.day}" ${d.playing ? "disabled" : ""}>${w.done ? "Continue" : "Play"}</button>` : ""}
      </div>`;
    }).join("")}</div>
    <div class="small muted">A day counts as done at 75% of its runs. The change compares the day's main drills with the same day last week. A missed day can be made up later in the week.</div>
  </section>`;
}

function summaryCard(d) {
  const s = d.progress.summary;
  const sat = d.days.find((x) => x.weak_spots);
  const satFocus = sat?.workout?.items.filter((i) => i.block === "main" || i.block === "secondary").map((i) => scenarioName(i.scenario)) ?? [];
  const notes = [
    s.lagging ? [`Furthest behind: ${SKILL_LABELS[s.lagging]}`, d.progress.profile.skills.find((x) => x.skill === s.lagging)?.runs ? "Your smallest gain since you started." : "Not trained yet."] : null,
    s.stale.length && s.stale.length < 9 ? ["Not trained in a week", `${s.stale.map((x) => SKILL_LABELS[x]).join(", ")}.`] : null,
    sat ? [`${sat.name}'s weak spots`, `${satFocus.length ? satFocus.join(", ") : "Built from your profile"}. Rebuilt from your skill profile, so they change as you improve.`] : null,
  ].filter(Boolean);
  return `<section class="card grow-1"><h2>Worth knowing</h2>
    ${notes.map(([t, b]) => `<div class="note"><div class="note-title">${esc(t)}</div><div class="note-body">${esc(b)}</div></div>`).join("") || `<div class="note-body">Notes show up here as the week goes on.</div>`}
  </section>`;
}

function profileCard(pr) {
  const max = Math.max(0.1, ...pr.skills.map((s) => Math.abs(s.change ?? 0)));
  const bar = (v) => {
    if (v === null || v === undefined) return `<span class="track"></span>`;
    const w = (Math.min(Math.abs(v), max) / max) * 50;
    return `<span class="track diverge"><span class="fill ${v < 0 ? "neg-fill" : ""}" style="${v < 0 ? `right:50%` : `left:50%`};width:${w}%"></span></span>`;
  };
  const groups = ["clicking", "tracking", "switching"];
  return `<section class="card grow-2"><div class="card-head"><h2>Skill profile</h2><span class="small muted">Change since you started each drill</span></div>
    <div class="kpis" style="grid-template-columns:repeat(auto-fit,minmax(120px,1fr));gap:12px">
      <div class="tile"><div class="small muted">Overall</div><div class="kpi-value">${pctChange(pr.overall)}</div></div>
      ${pr.groups.map((g) => `<div class="tile"><div class="small muted">${g.group[0].toUpperCase() + g.group.slice(1)}</div><div class="kpi-value">${pctChange(g.change)}</div></div>`).join("")}
    </div>
    ${groups.map((g) => `<div class="hbars">${pr.skills.filter((s) => s.group === g).map((s) => `<div class="hbar">
      <span class="lbl skill-lbl">${esc(SKILL_LABELS[s.skill])}</span>${bar(s.change)}
      <span class="val">${s.change !== null ? pctChange(s.change) : s.runs ? '<span class="muted small" title="Play it on another day to see a change">1st day</span>' : '<span class="muted small">new</span>'}</span></div>`).join("")}</div>`).join("")}
    <div class="small muted">Each drill's rating at the end of your latest day vs. the end of your first day on it (last three runs each, so the first run's search for your level doesn't count). Faster lifetimes and higher tracking speeds are both gains. A change shows once you've played a drill on two days.</div>
  </section>`;
}

function sessionsCard(sessions) {
  if (!sessions.length) {
    return `<section class="card"><h2>Reps and carry-over</h2><div class="note-body">Play a program day to see how each drill moves across its reps and how much sticks to the next session.</div></section>`;
  }
  return `<section class="card"><div class="card-head"><h2>Reps and carry-over</h2><span class="small muted">Most recent first</span></div>
    <div class="table-wrap"><table><thead><tr><th>When</th><th>Drill</th><th class="r">Reps</th><th class="r">Rep 1</th><th class="r">Last rep</th><th class="r">Within session</th><th class="r">Kept from last time</th></tr></thead><tbody>
    ${sessions.slice(0, 25).map((s) => `<tr><td style="white-space:nowrap">${esc(fmtShort(day(s.date)))}</td><td>${esc(s.scenario)} <span class="small muted">${esc(BLOCK_LABELS[s.block ?? "none"] || "")}</span></td>
      <td class="r mono">${s.reps}</td><td class="r mono">${esc(fmtKnob(s.knob, s.first))}</td><td class="r mono">${esc(fmtKnob(s.knob, s.last))}</td>
      <td class="r mono"><span class="${s.within >= 0 ? "pos" : "neg"}">${pctChange(s.within, 1)}</span></td>
      <td class="r mono">${s.carryover === null ? "–" : `<span class="${s.carryover >= 0 ? "pos" : "neg"}">${pctChange(s.carryover, 1)}</span>`}</td></tr>`).join("")}
    </tbody></table></div>
    <div class="small muted">"Within session" is your last rep vs. your first. "Kept from last time" compares your first rep with where the previous session ended. If you keep gaining within sessions but lose it by the next one, the skill isn't sticking yet: more sessions of the same drill will help.</div>
  </section>`;
}

async function startProgram(dayId) {
  await launchTrainer(() => invoke("play_program", { id: ui.program.data.id, day: dayId }));
  await loadProgram(true);
  if (ui.trainer.data) loadTrainer();
}

document.addEventListener("click", (e) => {
  const btn = e.target.closest('[data-action="play-program"]');
  if (btn) startProgram(btn.dataset.day);
});

// The week's runs changed. (trainer.js's listener shows any error it closed with.)
listen("trainer-closed", () => loadProgram(true));
