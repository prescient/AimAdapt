// Today: the home page. Today's program workout with each drill's runs and rating,
// the week at a glance, what you played last, and a couple of things worth knowing.
// Loaded after program.js and scenarios.js and draws with their helpers.

function today() {
  const tr = ui.trainer;
  if (!ui.program.data || !tr.data || !ui.insights.overview) {
    if (!ui.program.data) loadProgram();
    if (!tr.data) loadTrainer();
    if (!ui.insights.overview) loadInsightsOverview();
    return `<div class="empty-state">Loading today…</div>`;
  }
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
      <div style="flex:3 1 520px;min-width:0;display:flex;flex-direction:column;gap:16px">${todayWorkout()}</div>
      <div style="flex:2 1 300px;min-width:0;display:flex;flex-direction:column;gap:16px">${weekStrip()}${recentlyPlayed()}</div>
    </div>
    ${worthKnowing()}
  </div>`;
}

function todayWorkout() {
  const d = ui.program.data;
  const { today, next, stat } = todayInfo();
  const playing = d.playing || ui.trainer.data.playing;
  if (!today.workout) {
    return `<section class="card today-card"><div class="eyebrow">${esc(d.name)} · ${esc(today.name)}</div><h2>Rest day</h2>
      <div class="note-body">Next up: ${esc(next.name)}, ${esc(next.focus)}. If you want to play anyway, <a href="#scenarios">pick a scenario</a>.</div></section>`;
  }
  const done = stat?.status === "done";
  const started = stat && stat.done > 0;
  const byId = new Map(ui.trainer.data.scenarios.map((e) => [e.id, e]));
  const share = stat && stat.planned ? Math.min(1, stat.done / stat.planned) : 0;
  return `<section class="card today-card">
    <div class="card-head" style="align-items:flex-start">
      <div><div class="eyebrow accent">${esc(d.name)} · ${esc(today.name)}</div><h2 class="detail-title">${esc(today.focus)}</h2>
        <div class="small muted">${today.workout.items.length} drills · ${today.runs} runs · about ${Math.round(today.minutes)} min</div></div>
      <span class="chips"><a class="btn" href="#scenarios">Something else</a>
        <button class="btn primary" type="button" data-action="play-program" data-day="${today.day}" ${playing ? "disabled" : ""}>${playIcon}${done ? "Play again" : started ? "Continue" : "Start"}</button></span>
    </div>
    ${stat && stat.planned ? `<div class="day-progress"><div class="small muted"><span>${stat.done} of ${stat.planned} runs</span><span>${stat.minutes ? `${Math.round(stat.minutes)} min in` : ""}</span></div>
      <div class="bar-track"><div class="bar-fill" style="width:${(share * 100).toFixed(0)}%"></div></div></div>` : ""}
    <div class="drills" role="table" aria-label="Today's drills">
      <div class="drill head" role="row"><span role="columnheader">Block</span><span role="columnheader">Drill</span><span role="columnheader">Runs</span><span role="columnheader" class="r">85% rating</span></div>
      ${today.workout.items.map((it) => {
        const e = byId.get(it.scenario);
        const name = e?.scenario?.name ?? scenarioName(it.scenario);
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
  return `<section class="card"><div class="card-head"><h2>This week</h2><a class="small" href="#progress/skills">Week in detail</a></div>
    <div class="week-strip">${p.week.map((w) => {
      const info = d.days.find((x) => x.day === w.day);
      const label = w.planned ? (w.minutes ? `${Math.round(w.minutes)}` : w.status === "missed" ? "missed" : "–") : "rest";
      return `<div class="ws-day ${w.status} ${w.day === d.today ? "is-today" : ""}" title="${esc(`${info.name}: ${info.focus ?? "Rest"}${w.planned ? ` · ${w.done} of ${w.planned} runs` : ""}`)}">
        <span class="ws-name">${esc(info.name.slice(0, 3))}</span><span class="ws-cell">${esc(label)}</span></div>`;
    }).join("")}</div>
    <div class="small muted">Minutes per day in ${esc(d.name)}.</div>
  </section>`;
}

function recentlyPlayed() {
  const recent = scenarioItems().filter((x) => x.last).slice(0, 5);
  if (!recent.length) return "";
  return `<section class="card"><div class="card-head"><h2>Recently played</h2><a class="small" href="#scenarios">All scenarios</a></div>
    <div>${recent.map((x) => `<a class="list-row" href="${scenarioHref(x.key)}" style="color:inherit">
      <div class="main"><div class="title">${esc(x.name)}</div><div class="sub">${esc(dayLabel(x.last))}, ${esc(fmtTime(dt(x.last)))}${x.kind === "kovaaks" ? " · KovaaK's" : ""}</div></div>
      <div class="num">${esc(x.figure)}</div></a>`).join("")}</div>
  </section>`;
}

// At most two: the program's weakest skill, and the first pattern from your history.
function worthKnowing() {
  const s = ui.program.data.progress.summary;
  const notes = [];
  if (s.lagging) notes.push([`${SKILL_LABELS[s.lagging]} is furthest behind`, "Your smallest gain since you started. The program's weak-spots day leans on it.", "#progress/skills"]);
  const coach = ui.snap.training.runs.length ? coachNotes().filter((n) => n.title !== "Keep going") : [];
  if (coach.length) notes.push([coach[0].title, coach[0].body, null]);
  if (!notes.length) return "";
  return `<section class="card"><h2>Worth knowing</h2><div class="grid">${notes.slice(0, 2).map(([title, body, href]) => `<div class="note">
    <div class="note-title">${href ? `<a href="${href}">${esc(title)}</a>` : esc(title)}</div><div class="note-body">${esc(body)}</div></div>`).join("")}</div></section>`;
}
