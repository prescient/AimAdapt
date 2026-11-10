// The weekly training plan (Core Week): today's workout (on Today), the plan and its
// start buttons (Scenarios › Program), the results (Progress › Skills: the week, the
// skill profile and per-drill session gains) and the ranked notes Today and Progress
// both draw from. Loaded after trainer.js. Data comes from the `program` command (crates/aimadapt-core/src/programs.rs).

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
  if (["today", "progress", "scenarios"].includes(route().view)) render();
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

// The label on a program day's session button, from that day's stat (a week entry).
function sessionLabel(stat) {
  if (stat?.status === "done") return "Repeat session";
  return stat && stat.done > 0 ? "Continue session" : "Start session";
}

// A day's session button (Today and Scenarios › Program). Disabled while the trainer is open.
function sessionButton(dayId, stat, cls = "btn primary") {
  const playing = ui.program.data.playing || ui.trainer.data.playing;
  return `<button class="${cls}" type="button" data-action="play-program" data-day="${esc(dayId)}" ${playing ? "disabled" : ""}>${playIcon}${sessionLabel(stat)}</button>`;
}

// For a program drill (an item of a day's workout): { entry, name }, the trainer's
// entry for its scenario when there is one, and the name to show.
function drillLookup() {
  const byId = new Map(ui.trainer.data.scenarios.map((e) => [e.id, e]));
  return (it) => {
    const entry = byId.get(it.scenario);
    return { entry, name: entry?.scenario?.name ?? scenarioName(it.scenario) };
  };
}

// Every note, once, most important first. A note is { title, body } (plain text) and
// optionally one link: { skill } (opens Browse on that skill). Today shows the first;
// Progress › Skills lists them all.
function rankedNotes() {
  const s = ui.snap;
  const d = ui.program.data;
  const sum = d.progress.summary;
  const notes = [];
  const runs = s.training.runs.length;
  const sc = s.scenarios;
  if (sum.lagging) {
    const skill = SKILL_LABELS[sum.lagging];
    const trained = d.progress.profile.skills.find((x) => x.skill === sum.lagging)?.runs;
    notes.push({
      title: `${skill} is furthest behind`,
      body: `${trained ? "Your smallest gain since you started." : "Not trained yet."} The program's weak-spots day leans on it.`,
      skill,
    });
  }
  const plateaus = sc.filter((x) => x.plateau);
  if (runs && plateaus.length) {
    notes.push({
      title: `Plateau on ${plateaus.length === 1 ? plateaus[0].scenario : `${plateaus.length} scenarios`}`,
      body: `${s.progress_settings.plateau_runs}+ runs without a new best and a flat trend. Try an easier or harder variant for a while.`,
    });
  }
  const warm = s.cold_starts.map((c) => c.vs_rest_of_day_pct).filter((v) => v !== null);
  const warmMedian = median(warm);
  if (runs && warm.length >= 3 && warmMedian < -5) {
    notes.push({
      title: "Warm up first",
      body: `Your first run of the day on a scenario is typically ${Math.abs(warmMedian).toFixed(0)}% below your later runs that day (${warm.length} days measured). Start with an easy scenario.`,
    });
  }
  const perScenario = runs / Math.max(1, sc.length);
  if (runs && perScenario < 3) {
    const played = sc.filter((x) => x.counted_runs >= 5);
    const later = played.filter((x) => (x.improvement_pct ?? 0) > 0).length;
    notes.push({
      title: "Repeat before you switch",
      body: `${sc.length} scenarios in ${runs} runs is ${perScenario.toFixed(1)} runs each.` +
        (played.length ? ` In your ${played.length} scenarios with 5+ runs, the best came after the first attempt in ${later}.` : "") +
        " Try 3 to 5 runs per scenario.",
    });
  }
  if (sum.stale.length && sum.stale.length < 9) {
    notes.push({ title: "Not trained in a week", body: `${sum.stale.map((x) => SKILL_LABELS[x]).join(", ")}.` });
  }
  const periods = s.sens_periods;
  if (runs && periods.length > 1) {
    const [prev, cur] = periods.slice(-2);
    const cm = (p) => (p.cm_per_360 ? `${p.cm_per_360.toFixed(1)} cm/360` : sensLabel(p.settings));
    notes.push({
      title: `Settings changed on ${fmtShort(dt(cur.from))}`,
      body: `${cm(prev)} (${sensLabel(prev.settings)}) → ${cm(cur)} (${sensLabel(cur.settings)}). Score charts mark the change.`,
    });
  }
  const sat = d.days.find((x) => x.weak_spots);
  if (sat) {
    const focus = sat.workout?.items.filter((i) => i.block === "main" || i.block === "secondary").map((i) => scenarioName(i.scenario)) ?? [];
    notes.push({
      title: `${sat.name}'s weak spots`,
      body: `${focus.length ? focus.join(", ") : "Built from your profile"}. Rebuilt from your skill profile, so they change as you improve.`,
    });
  }
  return notes;
}

// One note, drawn. Titles and bodies are plain text: scenario names come from files.
function noteView(n) {
  const link = n.skill
    ? `<button type="button" class="link-btn inline" data-action="browse-skill" data-skill="${esc(n.skill)}">${esc(n.skill)} scenarios</button>`
    : "";
  return `<div class="note"><div class="note-title">${esc(n.title)}</div><div class="note-body">${esc(n.body)}${link ? ` ${link}` : ""}</div></div>`;
}

// Progress › Skills: how the week and each sub-skill are going. Results only: the plan
// and the start buttons are on Scenarios › Program.
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
      ${notesCard()}
    </div>
    ${sessionsCard(p.sessions)}
    ${weekCard(d)}`;
}

function weekCard(d) {
  const p = d.progress;
  return `<section class="card"><div class="card-head"><h2>This week</h2><span class="small muted">Week of ${esc(fmtShort(day(p.week_start)))} · <a href="#scenarios/program">The plan</a></span></div>
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
    <div class="small muted">A day counts as done at 75% of its runs. The change compares the day's main drills with the same day last week. Start or make up a day on the <a href="#scenarios/program">Program tab</a>.</div>
  </section>`;
}

// "Worth knowing" on Progress › Skills: the whole ranked list.
function notesCard() {
  const notes = rankedNotes();
  return `<section class="card grow-1"><h2>Worth knowing</h2>
    ${notes.map(noteView).join("") || `<div class="note-body">Notes show up here as the week goes on.</div>`}
  </section>`;
}

// Scenarios › Program: the plan. The header, each day with its drills and a session
// button, and how a day counts. Results are on Progress › Skills.
function programTab() {
  const st = ui.program;
  if (st.error) return `<div class="card error">${esc(st.error)}</div>`;
  const wait = waitFor([programNeed(), trainerNeed()], "Loading the program…");
  if (wait) return wait;
  const d = st.data;
  const p = d.progress;
  const s = p.summary;
  const drill = drillLookup();
  return `<section class="card">
      <div class="card-head"><div><div class="eyebrow accent">Program</div><h2 class="detail-title">${esc(d.name)}</h2></div>
        <span class="small muted">Week of ${esc(fmtShort(day(p.week_start)))}</span></div>
      <div class="note-body">${esc(d.description)}</div>
      <div><strong>${s.days_done} of ${s.days_planned}</strong> days done this week.</div>
      <div class="small"><a href="#help/program">How the program works</a> · <a href="#progress/skills">Your gains are on Progress › Skills</a></div>
    </section>
    <div class="day-grid">${p.week.map((w) => dayCard(d, w, d.days.find((x) => x.day === w.day), drill)).join("")}</div>
    <div class="small muted">A day counts as done at 75% of its runs. A missed day can be made up later in the week.</div>`;
}

// One day of the plan. `w` is the day's week entry, `info` the day itself.
function dayCard(d, w, info, drill) {
  const isToday = w.day === d.today;
  const head = `<div><div class="wd-name">${esc(info.name)} <span class="small muted">${esc(fmtShort(day(w.date)))}</span></div>
      <div class="small muted">${esc(info.focus ?? "Rest day")}</div></div>
      ${info.workout && STATUS_LABELS[w.status] ? `<span class="tag ${w.status === "done" || w.status === "today" ? "" : "grey"}">${STATUS_LABELS[w.status]}</span>` : ""}`;
  if (!info.workout) {
    return `<article class="card day-card rest ${isToday ? "is-today" : ""}"><div class="card-head">${head}</div></article>`;
  }
  const share = w.planned ? Math.min(1, w.done / w.planned) : 0;
  return `<article class="card day-card ${w.status} ${isToday ? "is-today" : ""}">
    <div class="card-head">${head}</div>
    <div class="small muted">${info.workout.items.length} drills · ${info.runs} runs · about ${Math.round(info.minutes)} min</div>
    ${info.weak_spots ? `<div class="small muted">Its drills are rebuilt from your skill profile, so they change as you improve.</div>` : ""}
    <ul class="day-drills">${info.workout.items.map((it) => {
      const { entry, name } = drill(it);
      return `<li><span class="small muted dd-block">${esc(BLOCK_LABELS[it.block ?? "none"] || "")}</span>
        <span class="dd-name">${entry?.scenario ? `<a href="${scenarioHref(entry.id)}">${esc(name)}</a>` : esc(name)}</span>
        <span class="muted dd-runs">×${it.runs}${it.mode === "standard" ? " · standard" : ""}</span></li>`;
    }).join("")}</ul>
    <div class="day-progress"><div class="small muted">${w.done} of ${w.planned} runs${w.minutes ? ` · ${Math.round(w.minutes)} min in` : ""}</div>
      <div class="bar-track"><div class="bar-fill" style="width:${(share * 100).toFixed(0)}%"></div></div></div>
    ${sessionButton(w.day, w, isToday ? "btn primary" : "btn")}
  </article>`;
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
