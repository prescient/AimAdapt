// Program: the weekly training plan (Core Week), today's workout, the week grid,
// the skill profile and per-drill session gains. Loaded after trainer.js. Data
// comes from the `program` command (crates/rustyaim-core/src/programs.rs).

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
    ui.program.data = tauri ? await invoke("program") : (await devData).program;
    ui.program.error = null;
  } catch (e) {
    ui.program.error = String(e);
  }
  const v = route().view;
  if (v === "program" || v === "play") render();
}

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

function workoutBlocks(w) {
  const groups = [];
  for (const it of w.items) {
    const label = BLOCK_LABELS[it.block ?? "none"] || "";
    let g = groups.find((x) => x.label === label);
    if (!g) groups.push((g = { label, items: [] }));
    g.items.push(`${esc(scenarioName(it.scenario))} ×${it.runs}${it.mode === "standard" ? " (standard)" : ""}`);
  }
  return `<div class="blocks">${groups.map((g) => `<div class="block-row"><span class="block-label">${esc(g.label)}</span><span>${g.items.join(", ")}</span></div>`).join("")}</div>`;
}

// Shown on the Play screen.
function todayCard() {
  if (!ui.program.data) {
    loadProgram();
    return "";
  }
  const d = ui.program.data;
  const { today, next, stat } = todayInfo();
  const playing = d.playing || ui.trainer.data?.playing;
  if (!today.workout) {
    return `<section class="card today-card"><div class="card-head"><h2>Today: rest day</h2><a class="btn" href="#program">View program</a></div>
      <div class="note-body">${esc(d.name)} has no workout on ${esc(today.name)}. Next up: ${esc(next.name)}, ${esc(next.focus)}.</div></section>`;
  }
  const done = stat && stat.status === "done";
  return `<section class="card today-card">
    <div class="card-head" style="align-items:center"><div><div class="eyebrow">${esc(d.name)} · ${esc(today.name)}</div><h2>${esc(today.focus)}</h2></div>
      <span class="chips"><a class="btn" href="#program">View program</a>
      <button class="btn primary" type="button" data-action="play-program" data-day="${today.day}" ${playing ? "disabled" : ""}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M7 4l13 8-13 8z"></path></svg>${done ? "Play again" : stat && stat.done ? "Continue" : "Start"}</button></span></div>
    <div class="small muted">${today.runs} runs · about ${Math.round(today.minutes)} minutes${stat && stat.done ? ` · ${stat.done} of ${stat.planned} done today` : ""}</div>
    ${workoutBlocks(today.workout)}
  </section>`;
}

function program() {
  const st = ui.program;
  if (st.error) return `<div class="page">${head("Training program", "Program")}<div class="card error">${esc(st.error)}</div></div>`;
  if (!st.data || !ui.trainer.data) {
    loadProgram();
    if (!ui.trainer.data) loadTrainer();
    return `<div class="empty-state">Loading program…</div>`;
  }
  const d = st.data;
  const p = d.progress;
  return `<div class="page">
    ${head("Training program", esc(d.name))}
    <div class="note-body">${esc(d.description)}</div>
    ${todayCard()}
    ${weekCard(d)}
    <div class="row">
      ${summaryCard(d)}
      ${profileCard(p.profile)}
    </div>
    ${sessionsCard(p.sessions)}
    <section class="card"><h2>The week</h2>
      <div class="grid">${d.days.filter((x) => x.workout).map((x) => `<div class="note"><div class="note-title">${esc(x.name)}: ${esc(x.focus)}</div>
        <div class="small muted">${x.runs} runs · about ${Math.round(x.minutes)} min${x.weak_spots ? " · rebuilt from your skill profile" : ""}</div>
        ${workoutBlocks(x.workout)}
        <div><button class="btn" type="button" data-action="play-program" data-day="${x.day}" ${d.playing ? "disabled" : ""}>Play ${esc(x.name)}</button></div></div>`).join("")}</div>
    </section>
  </div>`;
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
      </div>`;
    }).join("")}</div>
    <div class="small muted">A day counts as done at 75% of its runs. The change compares the day's main drills with the same day last week.</div>
  </section>`;
}

function summaryCard(d) {
  const s = d.progress.summary;
  const sat = d.days.find((x) => x.weak_spots);
  const satFocus = sat?.workout?.items.filter((i) => i.block === "main" || i.block === "secondary").map((i) => scenarioName(i.scenario)) ?? [];
  return `<section class="card grow-1"><h2>Weekly summary</h2>
    <div class="kpis" style="grid-template-columns:repeat(auto-fit,minmax(120px,1fr));gap:12px">
      <div class="tile"><div class="small muted">Days done</div><div class="kpi-value">${s.days_done} / ${s.days_planned}</div></div>
      <div class="tile"><div class="small muted">Minutes</div><div class="kpi-value">${Math.round(s.minutes)}</div></div>
      <div class="tile"><div class="small muted">Streak</div><div class="kpi-value">${s.streak}</div></div>
    </div>
    ${s.best ? `<div class="note"><div class="note-title">Biggest gain: ${esc(SKILL_LABELS[s.best[0]])}</div><div class="note-body">${pctChange(s.best[1], 1)} over the last 7 days.</div></div>` : ""}
    ${s.lagging ? `<div class="note"><div class="note-title">Furthest behind: ${esc(SKILL_LABELS[s.lagging])}</div><div class="note-body">${d.progress.profile.skills.find((x) => x.skill === s.lagging)?.runs ? "Your smallest gain since you started." : "Not trained yet."}</div></div>` : ""}
    ${s.stale.length && s.stale.length < 9 ? `<div class="note"><div class="note-title">Not trained in a week</div><div class="note-body">${s.stale.map((x) => esc(SKILL_LABELS[x])).join(", ")}.</div></div>` : ""}
    ${sat ? `<div class="note"><div class="note-title">Saturday's weak spots</div><div class="note-body">${satFocus.length ? esc(satFocus.join(", ")) : "Built from your profile."} These are rebuilt from your skill profile, so they change as you improve.</div></div>` : ""}
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
  try {
    await invoke("play_program", { id: ui.program.data.id, day: dayId });
    ui.trainer.message = null;
  } catch (e) {
    ui.trainer.message = String(e);
  }
  await loadProgram(true);
  if (ui.trainer.data) loadTrainer();
}

document.addEventListener("click", (e) => {
  const btn = e.target.closest('[data-action="play-program"]');
  if (btn) startProgram(btn.dataset.day);
});

if (tauri) tauri.event.listen("trainer-closed", () => loadProgram(true));
