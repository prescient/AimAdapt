// Insights: analytics for runs played in AimAdapt's own trainer, drawn on each
// scenario's page (scenarios.js). Loaded after app.js and trainer.js. Everything comes
// from `insights_overview` and `insights` (crates/aimadapt-core/src/insights.rs);
// this file only draws it.

ui.insights = { overview: null, cache: new Map(), error: null };

async function loadInsightsOverview(force = false) {
  if (ui.insights.overview && !force) return;
  try {
    ui.insights.overview = (await invoke("insights_overview")).scenarios;
    ui.insights.cache.clear();
    ui.insights.error = null;
  } catch (e) {
    ui.insights.error = String(e);
  }
  if (["scenarios", "today"].includes(route().view)) render();
}

// What views that need the overview pass to waitFor (app.js).
const overviewNeed = () => ({ key: "insights-overview", have: Boolean(ui.insights.overview), error: ui.insights.error, load: loadInsightsOverview });

// One scenario's insights: undefined while they load, null if there are none, or
// { error }. Loaded once, however often the page asks (see loadCached).
const insightsFor = (name) => loadCached(ui.insights.cache, name, () => invoke("insights", { scenario: name }));

const ms = (v) => (v === null || v === undefined ? "–" : `${Math.round(v)} ms`);
const share = (v) => (v === null || v === undefined ? "–" : `${Math.round(v * 100)}%`);
const delta = (now, before, unit = "", better = 1) => {
  if (now === null || now === undefined || before === null || before === undefined) return "";
  const d = now - before;
  const good = d * better > 0;
  return `<span class="${good ? "pos" : "neg"}">${d > 0 ? "+" : ""}${unit === "%" ? Math.round(d * 100) : d.toFixed(unit === "bits/s" ? 1 : 0)}${unit === "%" ? " pts" : unit ? ` ${unit}` : ""}</span> vs. runs before`;
};

// The scenario's ratings that are on one scale: adaptive runs with its current rated
// setting and its current version. A rating from before the scenario's settings changed
// (`current` false) is on another scale, so the bests and the chart leave it out.
const ratedRuns = (d) => d.runs.filter((r) => r.current && r.adaptive && r.rating !== null && r.knob === d.knob);

// A run's rating for a table. One from another version of the scenario is greyed.
const OLD_VERSION = "From before this scenario's settings changed, so it isn't compared with your current rating";
const ratingCell = (r) => (r.rating === null ? "–"
  : r.current ? esc(fmtKnob(r.knob, r.rating))
  : `<span class="muted" title="${OLD_VERSION}">${esc(fmtKnob(r.knob, r.rating))}</span>`);

// Up to eight key figures for a scenario, most important first. The rating tile
// is left out when the page's header already shows the rating (withRating false).
function insightKpis(d, withRating = true) {
  const adaptive = ratedRuns(d);
  const latest = adaptive.at(-1);
  const best = adaptive.reduce((m, r) => (m === null || betterThan(d.knob, r.rating, m) ? r.rating : m), null);
  const fastest = adaptive.reduce((m, r) => (r.fastest !== null && (m === null || betterThan(d.knob, r.fastest, m)) ? r.fastest : m), null);
  const t = d.tracking;
  const sw = d.switching;
  return [
    latest && withRating ? kpi(`85% ${knobWord(d.knob)}`, esc(fmtKnob(d.knob, latest.rating)), `best ${esc(fmtKnob(d.knob, best))}${fastest !== null ? ` · held ${esc(fmtKnob(d.knob, fastest))}` : ""}`) : "",
    d.success !== null ? kpi("Targets hit", share(d.success), `destroyed before timing out · last ${d.recent_runs} runs`) : "",
    t && !sw ? kpi("On target", share(t.on_target), delta(t.on_target, t.on_target_before, "%") || `median miss ${t.median_error_deg.toFixed(2)}°`) : "",
    d.reaction ? kpi("Reaction", `${Math.round(d.reaction.median_ms)}<small> ms</small>`, delta(d.reaction.median_ms, d.reaction.median_before_ms, "ms", -1) || "median, before moving") : "",
    d.fitts ? kpi("Flick speed", `${d.fitts.throughput.toFixed(1)}<small> bits/s</small>`, delta(d.fitts.throughput, d.fitts.throughput_before, "bits/s") || "speed for size and distance") : "",
    sw && sw.kills_per_min !== null ? kpi("Kills a minute", `${Math.round(sw.kills_per_min)}`, delta(sw.kills_per_min, sw.kills_per_min_before, "") || "overall switching pace") : "",
    sw && sw.acquire_ms !== null ? kpi("Acquire", `${Math.round(sw.acquire_ms)}<small> ms</small>`, delta(sw.acquire_ms, sw.acquire_before_ms, "ms", -1) || "kill to reaching the next target, median") : "",
    sw && sw.engaged_on_target !== null ? kpi("On target while killing", share(sw.engaged_on_target), delta(sw.engaged_on_target, sw.engaged_on_target_before, "%") || "time between targets not counted") : "",
  ].filter(Boolean);
}

// Rows the Runs table shows until Show all.
const RUNS_SHOWN = 30;

// What a run's kind is called. Standard runs have no plan; an adaptive run saved before
// kinds were planned has no pick and reads as plain Adaptive.
const KIND_LABELS = { calibrate: "Calibration", near: "At your level", stretch: "Stretch", easy: "Easy" };
const runKind = (r) => (!r.adaptive ? "Standard" : KIND_LABELS[r.pick] ?? "Adaptive");

// "Planned against actual": how often you succeeded on each kind of run beside what the
// trainer planned. `plan` is a PlanCheck; a missing one reads as no runs.
const PLAN_KINDS = ["near", "stretch", "easy"];
function gapCell(k) {
  if (k.gap === null || k.gap === undefined) return "–";
  const points = Math.round(k.gap * 100);
  const text = `${points > 0 ? "+" : ""}${points} pts`;
  if (k.verdict === "above") return `<strong class="pos">${text} above plan</strong>`;
  if (k.verdict === "below") return `<strong class="neg">${text} below plan</strong>`;
  return `<span class="muted">${text}</span>`;
}
function planCheckCard(plan) {
  const kinds = PLAN_KINDS.map((id) => (plan?.kinds ?? []).find((k) => k.kind === id)
    ?? { kind: id, runs: 0, planned: null, actual: null, gap: null, verdict: null });
  const uncounted = plan?.uncounted ?? 0;
  const lead = `<div class="note-body">How often you succeeded on each kind of run, beside what the trainer planned.</div>`;
  const head = `<h2>Planned against actual</h2>`;
  if (!kinds.some((k) => k.runs > 0)) {
    return `<section class="card">${head}${lead}
      <div class="note-body">No planned runs yet. A calibration run has no plan, so this starts with the next run after it.</div>
      ${uncounted ? uncountedNote(uncounted) : ""}</section>`;
  }
  return `<section class="card">${head}${lead}
    <div class="table-wrap"><table><thead><tr><th>Kind</th><th class="r">Runs</th><th class="r">Planned</th><th class="r">Actual</th><th class="r">Gap</th></tr></thead><tbody>
      ${kinds.map((k) => `<tr><td>${KIND_LABELS[k.kind]}</td><td class="r mono">${k.runs}</td><td class="r mono">${share(k.planned)}</td><td class="r mono">${share(k.actual)}</td><td class="r mono" style="white-space:nowrap">${gapCell(k)}</td></tr>`).join("")}
    </tbody></table></div>
    <div class="small muted" style="margin-top:8px">A gap is marked when it is bigger than luck explains for that many targets, and never under five points.</div>
    ${uncounted ? uncountedNote(uncounted) : ""}</section>`;
}
const uncountedNote = (n) => `<div class="small muted">${n === 1 ? "1 earlier run is" : `${n} earlier runs are`} left out because their length was not recorded.</div>`;

// The latest runs (all of them after Show all), each with a Replay link when it
// still has one: the latest 50 and the best keep theirs.
function insightRunsTable(d) {
  const rows = [...d.runs].reverse();
  const shown = ui.allRuns ? rows : rows.slice(0, RUNS_SHOWN);
  return `<div class="card-head"><h2>Runs</h2><span class="small muted">${d.runs.length} in all · newest first</span></div>
    <div class="table-wrap"><table><thead><tr><th>When</th><th>Kind</th><th class="r">85% rating</th><th class="r">Hit</th><th class="r">Score</th><th class="r">Flick speed</th><th class="r">Reaction</th><th class="r">On target</th><th>Played in</th><th></th></tr></thead><tbody>
      ${shown.map((r) => `<tr><td style="white-space:nowrap">${esc(fmtShort(dt(r.finished_at)))}, ${esc(fmtTime(dt(r.finished_at)))}</td><td style="white-space:nowrap">${runKind(r)}</td>
        <td class="r num" style="white-space:nowrap">${r.breakthrough ? `<span class="tag gold" title="Held a harder level than any earlier run of this version">Breakthrough</span> ` : ""}${ratingCell(r)}</td><td class="r mono">${share(r.success_rate ?? r.accuracy)}</td><td class="r mono">${r.adaptive ? "–" : num(r.score)}</td>
        <td class="r mono">${r.throughput ? `${r.throughput.toFixed(1)}` : "–"}</td><td class="r mono">${ms(r.reaction_ms)}</td><td class="r mono">${share(r.on_target)}</td><td class="muted">${esc(r.workout ?? "")}</td>
        <td>${r.replay ? `<a href="#replay/${r.run_id}" title="Watch this run again">Replay</a>` : ""}</td></tr>`).join("")}
    </tbody></table></div>
    ${shown.some((r) => r.rating !== null && !r.current) ? `<div class="small muted" style="margin-top:8px">Greyed ratings are from before this scenario's settings changed. They're on another scale, so your best, the rating chart and the notes leave them out.</div>` : ""}
    ${rows.length > RUNS_SHOWN ? `<button type="button" class="link-btn" data-action="all-runs">${ui.allRuns ? `Show the latest ${RUNS_SHOWN}` : `Show all ${rows.length} runs`}</button>` : ""}`;
}

// The scenario header's one line of figures: the latest rating with its likely range,
// and the best held. Empty with no adaptive runs.
function ratingLine(d, adaptive) {
  const latest = adaptive.at(-1);
  if (!latest) return "";
  const fastest = adaptive.reduce((m, r) => (r.fastest !== null && (m === null || betterThan(d.knob, r.fastest, m)) ? r.fastest : m), null);
  const low = latest.rating_low, high = latest.rating_high;
  const range = low !== null && low !== undefined && high !== null && high !== undefined
    ? ` (likely ${fmtKnob(d.knob, low).split(" ")[0]}–${fmtKnob(d.knob, high)})` : "";
  const held = fastest !== null ? ` · best held ${fmtKnob(d.knob, fastest)}` : "";
  return `85% ${knobWord(d.knob)} ${fmtKnob(d.knob, latest.rating)}${range}${held}`;
}

// "What to work on": the first `limit` notes, or all of them. `moreHref` links to the
// rest when some are left out.
function workOnCard(d, limit = Infinity, moreHref = "") {
  const shown = d.notes.slice(0, limit);
  return `<section class="card"><h2>What to work on</h2>
    ${shown.length ? `<div class="grid">${shown.map((n) => `<div class="note"><div class="note-title">${esc(n.title)}</div><div class="note-body">${esc(n.body)}</div></div>`).join("")}</div>` : `<div class="note-body">Play a few more runs for notes on what to work on.</div>`}
    ${shown.length < d.notes.length && moreHref ? `<a class="small" href="${moreHref}">All ${d.notes.length} notes on the Insights tab</a>` : ""}
  </section>`;
}

// The latest five runs with their Replay links, and a link to the whole table.
function latestRunsCard(d, runsHref) {
  const rows = [...d.runs].reverse().slice(0, 5);
  return `<section class="card"><div class="card-head"><h2>Latest runs</h2><a class="small" href="${runsHref}">All ${d.runs.length} runs</a></div>
    <div class="table-wrap"><table><thead><tr><th>When</th><th>Kind</th><th class="r">85% rating</th><th class="r">Hit</th><th></th></tr></thead><tbody>
      ${rows.map((r) => `<tr><td>${esc(fmtShort(dt(r.finished_at)))}, ${esc(fmtTime(dt(r.finished_at)))}</td><td>${runKind(r)}</td>
        <td class="r num">${ratingCell(r)}</td><td class="r mono">${share(r.success_rate ?? r.accuracy)}</td>
        <td>${r.replay ? `<a href="#replay/${r.run_id}" title="Watch this run again">Replay</a>` : ""}</td></tr>`).join("")}
    </tbody></table></div></section>`;
}

// ---------- charts ----------

// The charts here and the score chart (app.js) share these. A frame is a chart's
// viewBox size (w × h) and the room left for its axes (l, r, t, b); sx and sy place
// a value from lo to hi across or up a frame's plot area, CHART's unless given.
const CHART = { w: 560, h: 240, l: 44, r: 14, t: 14, b: 30 };
const sx = (v, lo, hi, f = CHART) => f.l + ((v - lo) / (hi - lo || 1)) * (f.w - f.l - f.r);
const sy = (v, lo, hi, f = CHART) => f.h - f.b - ((v - lo) / (hi - lo || 1)) * (f.h - f.t - f.b);
const tickValues = (lo, hi, n = 4) => Array.from({ length: n + 1 }, (_, i) => lo + ((hi - lo) * i) / n);

// Grid lines and labels for n + 1 evenly spaced values from lo to hi. yfmt gets the
// exact tick value: rounding first made close ticks read the same.
function yTicks(lo, hi, yfmt = (v) => Math.round(v * 10) / 10, n = 4, f = CHART) {
  return tickValues(lo, hi, n).map((v) => `<line x1="${f.l}" x2="${f.w - f.r}" y1="${sy(v, lo, hi, f)}" y2="${sy(v, lo, hi, f)}" stroke="var(--border)" />
      <text x="${f.l - 6}" y="${sy(v, lo, hi, f) + 4}" text-anchor="end" font-size="11" fill="var(--dim)">${yfmt(v)}</text>`).join("");
}

// Both axes of a CHART-sized chart, with their labels.
function axes(xlo, xhi, ylo, yhi, xlabel, ylabel, yfmt, yn) {
  const f = CHART;
  return `${yTicks(ylo, yhi, yfmt, yn)}
    ${tickValues(xlo, xhi).map((v) => `<text x="${sx(v, xlo, xhi)}" y="${f.h - f.b + 16}" text-anchor="middle" font-size="11" fill="var(--dim)">${Math.round(v * 10) / 10}</text>`).join("")}
    <text x="${f.w - f.r}" y="${f.h - 2}" text-anchor="end" font-size="11" fill="var(--dim)">${xlabel}</text>
    <text x="${f.l}" y="${f.t - 2}" font-size="11" fill="var(--dim)">${ylabel}</text>`;
}

const svg = (body, h = CHART.h, w = CHART.w) => `<svg viewBox="0 0 ${w} ${h}" width="100%" role="img" style="max-height:${h + 40}px">${body}</svg>`;

// Rating by run, drawn so "better" is always up: for lifetime and size the axis is
// flipped, since lower is better.
function ratingCard(d, adaptive) {
  const knob = d.knob;
  const up = higherIsHarder(knob);
  const vals = adaptive.flatMap((r) => [r.rating, r.rating_low ?? r.rating, r.rating_high ?? r.rating]);
  const pad = (Math.max(...vals) - Math.min(...vals)) * 0.15 || Math.max(...vals) * 0.1;
  let lo = Math.min(...vals) - pad, hi = Math.max(...vals) + pad;
  if (!up) [lo, hi] = [hi, lo];
  const n = Math.max(1, adaptive.length - 1);
  const fmtTick = (v) => fmtKnob(knob, v).replace(" °/s", "").replace(" s", "").replace(" m", "");
  const band = adaptive.map((r, i) => `${sx(i, 0, n)},${sy(r.rating_high ?? r.rating, lo, hi)}`).join(" ") + " " +
    [...adaptive].reverse().map((r, j) => `${sx(adaptive.length - 1 - j, 0, n)},${sy(r.rating_low ?? r.rating, lo, hi)}`).join(" ");
  const line = adaptive.map((r, i) => `${i ? "L" : "M"}${sx(i, 0, n)},${sy(r.rating, lo, hi)}`).join("");
  const tl = d.latest_timeline;
  let tchart = "";
  if (tl.length > 2) {
    const tmax = tl.at(-1)[0] || 60;
    const tv = tl.map((p) => p[1]);
    let tlo = Math.min(...tv), thi = Math.max(...tv);
    const tp = (thi - tlo) * 0.1 || thi * 0.05;
    tlo -= tp;
    thi += tp;
    if (!up) [tlo, thi] = [thi, tlo];
    const tline = tl.map((p, i) => `${i ? "L" : "M"}${sx(p[0], 0, tmax)},${sy(p[1], tlo, thi)}`).join("");
    tchart = `<section class="card grow-1"><div class="card-head"><h2>Last run</h2><span class="small muted">Your rating as the run went on</span></div>
      ${svg(`${axes(0, tmax, tlo, thi, "seconds", knob === "target_speed" ? "°/s" : knob === "lifetime" || knob === "kill_pace" ? "s" : "m", fmtTick)}<path d="${tline}" fill="none" stroke="var(--blue)" stroke-width="2"/>`)}
      <div class="small muted">The rating takes in each target or segment as you play. A calibration run moves the difficulty with it; later runs stay at one value and the rating settles as it gets more certain.</div></section>`;
  }
  return `<div class="row">
    <section class="card grow-2"><div class="card-head"><h2>85% ${knobWord(knob)} by run</h2><span class="small muted">Up is better · shaded = likely range</span></div>
      ${svg(`${axes(1, n + 1, lo, hi, "run", knob === "target_speed" ? "°/s" : knob === "lifetime" || knob === "kill_pace" ? "s" : "m", fmtTick)}
        <polygon points="${band}" fill="var(--rust)" opacity="0.15"/>
        <path d="${line}" fill="none" stroke="var(--rust)" stroke-width="2.5"/>
        ${adaptive.map((r, i) => `<circle cx="${sx(i, 0, n)}" cy="${sy(r.rating, lo, hi)}" r="3.5" fill="var(--rust)"><title>Run ${i + 1}: ${esc(fmtKnob(knob, r.rating))}</title></circle>`).join("")}`)}
    </section>
    ${tchart}
  </div>`;
}

// Chance of success at each speed, from the model, with what actually happened.
function curveCard(c) {
  const knob = c.knob;
  const h = (v) => (higherIsHarder(knob) ? Math.log(v) : -Math.log(v));
  const xs = c.model.map((p) => h(p[0]));
  const xlo = Math.min(...xs), xhi = Math.max(...xs);
  const X = (v) => sx(h(v), xlo, xhi);
  const line = c.model.map((p, i) => `${i ? "L" : "M"}${X(p[0])},${sy(p[1] * 100, 0, 100)}`).join("");
  const f = CHART;
  // Labels sit low on the chart, staggered, so they don't cover the curve's top.
  const mark = (v, label, row) => `<line x1="${X(v)}" x2="${X(v)}" y1="${f.t}" y2="${f.h - f.b}" stroke="var(--dim)" stroke-dasharray="3 4"/>
    <text x="${X(v) + 4}" y="${f.h - f.b - 8 - row * 14}" font-size="11" fill="var(--muted)">${label} ${esc(fmtKnob(knob, v))}</text>`;
  const xTicks = [0, 0.25, 0.5, 0.75, 1].map((k) => {
    const v = c.model[Math.round(k * (c.model.length - 1))][0];
    return `<text x="${X(v)}" y="${f.h - f.b + 16}" text-anchor="middle" font-size="11" fill="var(--dim)">${esc(fmtKnob(knob, v))}</text>`;
  }).join("");
  return `<section class="card"><div class="card-head"><h2>Speed vs. hit rate</h2><span class="small muted">Harder to the right</span></div>
    ${svg(`${yTicks(0, 100, (v) => `${v}%`)}
      ${xTicks}
      ${mark(c.at95, "95%", 2)}${mark(c.at85, "85%", 1)}${mark(c.at50, "50%", 0)}
      <path d="${line}" fill="none" stroke="var(--rust)" stroke-width="2.5"/>
      ${c.observed.map((o) => `<circle cx="${X(o[0])}" cy="${sy(o[1] * 100, 0, 100)}" r="${3 + Math.min(6, Math.sqrt(o[2]))}" fill="var(--blue)" opacity="0.6"><title>${esc(fmtKnob(knob, o[0]))}: ${Math.round(o[1] * 100)}% of ${o[2]} targets</title></circle>`).join("")}`)}
    <div class="note-body">The line is your current curve: the chance of hitting a target at each ${({ target_speed: "speed", lifetime: "lifetime", kill_pace: "kill pace" })[knob] ?? "size"}. ${c.observed.length ? "Blue dots are what actually happened in recent runs (bigger = more targets). " : ""}You're reliable (95%) at ${esc(fmtKnob(knob, c.at95))} and it's a coin flip at ${esc(fmtKnob(knob, c.at50))}. A steep drop means you're consistent up to your limit; a gentle slope means misses creep in early.</div>
  </section>`;
}

function fittsCard(f) {
  const xs = f.points.map((p) => p.id_bits), ys = f.points.map((p) => p.time_ms);
  const xlo = 0, xhi = Math.ceil(Math.max(...xs, 1));
  const ylo = 0, yhi = Math.ceil(Math.max(...ys, 400) / 200) * 200;
  const fit = (x) => f.intercept_ms + f.slope_ms_per_bit * x;
  return `<section class="card"><div class="card-head"><h2>Speed vs. difficulty</h2><span class="small muted">${f.points.length} flicks</span></div>
    ${svg(`${axes(xlo, xhi, ylo, yhi, "difficulty (bits)", "ms to kill")}
      ${f.points.map((p) => `<circle cx="${sx(p.id_bits, xlo, xhi)}" cy="${sy(Math.min(p.time_ms, yhi), ylo, yhi)}" r="2.5" fill="var(--blue)" opacity="0.45"/>`).join("")}
      <line x1="${sx(xlo, xlo, xhi)}" y1="${sy(fit(xlo), ylo, yhi)}" x2="${sx(xhi, xlo, xhi)}" y2="${sy(fit(xhi), ylo, yhi)}" stroke="var(--rust)" stroke-width="2.5"/>`)}
    <div class="note-body">Each dot is a kill. Difficulty combines distance and size (Fitts' law: log₂(distance ÷ width + 1)). The line is your fit: ${Math.round(f.intercept_ms)} ms + ${Math.round(f.slope_ms_per_bit)} ms per bit. A lower line means faster flicks; a flatter one means distance and size cost you less.</div>
  </section>`;
}

function directionCard(dirs) {
  const cx = 150, cy = 150, R = 110;
  const times = dirs.map((d) => d.median_ms).filter((v) => v);
  const tlo = Math.min(...times), thi = Math.max(...times);
  const wedge = (d) => {
    const a0 = ((d.direction_deg - 22.5) * Math.PI) / 180, a1 = ((d.direction_deg + 22.5) * Math.PI) / 180;
    const r = 20 + (R - 20) * d.success;
    const p = (a, rr) => `${cx + rr * Math.cos(a)},${cy - rr * Math.sin(a)}`;
    const k = d.median_ms && thi > tlo ? (d.median_ms - tlo) / (thi - tlo) : 0;
    const fill = `color-mix(in srgb, var(--rust) ${Math.round(100 - k * 60)}%, var(--session))`;
    return `<path d="M${cx},${cy} L${p(a0, r)} A${r},${r} 0 0,0 ${p(a1, r)} Z" fill="${fill}" stroke="var(--surface)" stroke-width="2"><title>${d.label}: ${Math.round(d.success * 100)}% hit, ${ms(d.median_ms)}, ${d.targets} targets</title></path>
      <text x="${cx + (R + 22) * Math.cos((d.direction_deg * Math.PI) / 180)}" y="${cy - (R + 22) * Math.sin((d.direction_deg * Math.PI) / 180) + 4}" text-anchor="middle" font-size="11" fill="var(--muted)">${Math.round(d.success * 100)}%</text>`;
  };
  return `<section class="card"><div class="card-head"><h2>By flick direction</h2><span class="small muted">From the crosshair to the target</span></div>
    <div style="display:flex;flex-wrap:wrap;gap:12px;align-items:center">
      ${svg(`<circle cx="${cx}" cy="${cy}" r="${R}" fill="none" stroke="var(--border)"/><circle cx="${cx}" cy="${cy}" r="${20 + (R - 20) * 0.5}" fill="none" stroke="var(--border)" stroke-dasharray="3 4"/>${dirs.map(wedge).join("")}`, 300, 300).replace('width="100%"', 'width="300"')}
      <div class="table-wrap" style="flex:1 1 200px"><table><thead><tr><th>Direction</th><th class="r">Hit</th><th class="r">Median</th><th class="r">Overshoot</th></tr></thead><tbody>
        ${dirs.map((d) => `<tr><td>${d.label}</td><td class="r mono">${share(d.success)}</td><td class="r mono">${ms(d.median_ms)}</td><td class="r mono">${d.overshoot_deg.toFixed(2)}°</td></tr>`).join("")}
      </tbody></table></div>
    </div>
    <div class="small muted">Wedge length is the share of targets hit (dashed ring = 50%). Brighter wedges are faster.</div>
  </section>`;
}

function shotCard(m) {
  const S = 260, c = S / 2, k = (S / 2 - 10) / 4; // 4 target radii to the edge
  const dot = (p) => `<circle cx="${c + Math.max(-4, Math.min(4, p[0])) * k}" cy="${c - Math.max(-4, Math.min(4, p[1])) * k}" r="2.6" fill="${p[2] ? "var(--blue)" : "var(--rust)"}" opacity="${p[2] ? 0.35 : 0.8}"/>`;
  const lines = [];
  if (m.overshoot_share !== null) lines.push(`${Math.round(m.overshoot_share * 100)}% of misses went past the target, ${Math.round((1 - m.overshoot_share) * 100)}% stopped short.`);
  if (m.vertical_deg !== null) lines.push(`On average your shots land ${Math.abs(m.vertical_deg).toFixed(2)}° ${m.vertical_deg >= 0 ? "high" : "low"} and ${Math.abs(m.horizontal_deg ?? 0).toFixed(2)}° ${(m.horizontal_deg ?? 0) >= 0 ? "right" : "left"}.`);
  return `<section class="card"><div class="card-head"><h2>Where shots land</h2><span class="small muted">${m.points.length} shots</span></div>
    <div style="display:flex;flex-wrap:wrap;gap:12px;align-items:center">
      ${svg(`<line x1="0" x2="${S}" y1="${c}" y2="${c}" stroke="var(--border)"/><line y1="0" y2="${S}" x1="${c}" x2="${c}" stroke="var(--border)"/>
        <circle cx="${c}" cy="${c}" r="${k}" fill="none" stroke="var(--text)" stroke-width="1.5"/>
        <path d="M${c - 3 * k},${S - 14} L${c + 3 * k},${S - 14}" stroke="var(--dim)" stroke-width="1.5" marker-end="url(#arr)"/>
        <defs><marker id="arr" markerWidth="8" markerHeight="8" refX="6" refY="4" orient="auto"><path d="M0,0 L8,4 L0,8 z" fill="var(--dim)"/></marker></defs>
        <text x="${c}" y="${S - 20}" text-anchor="middle" font-size="11" fill="var(--dim)">flick direction</text>
        ${m.points.map(dot).join("")}`, S, S).replace('width="100%"', `width="${S}"`)}
      <div class="note-body" style="flex:1 1 200px">The circle is the target. Every shot is turned so the flick went left to right, so dots to the right of the circle overshot and dots to the left stopped short. Blue = hit, orange = miss.<br><br>${lines.join("<br>")}</div>
    </div>
  </section>`;
}

function bars(items, fmt, max = 1) {
  return `<div class="hbars">${items.map(([label, v, note]) => `<div class="hbar"><span class="lbl wide">${esc(label)}</span><span class="track"><span class="fill" style="width:${Math.max(0, Math.min(1, v / max)) * 100}%"></span></span><span class="val">${fmt(v)}</span>${note ? `<span class="small muted">${esc(note)}</span>` : ""}</div>`).join("")}</div>`;
}

function reactionCard(r) {
  const f = CHART;
  const max = Math.max(...r.histogram, 1);
  const bw = (f.w - f.l - f.r) / r.histogram.length;
  return `<section class="card"><div class="card-head"><h2>Reaction time</h2><span class="small muted">Until the crosshair starts moving</span></div>
    ${svg(`${axes(0, 600, 0, max, "ms", "targets", (v) => Math.round(v))}${r.histogram.map((n, i) => `<rect x="${f.l + i * bw + 2}" y="${sy(n, 0, max)}" width="${bw - 4}" height="${f.h - f.b - sy(n, 0, max)}" fill="var(--rust)" rx="2"><title>${i * 50}–${i * 50 + 50} ms: ${n}</title></rect>`).join("")}
      <line x1="${sx(r.median_ms, 0, 600)}" x2="${sx(r.median_ms, 0, 600)}" y1="${f.t}" y2="${f.h - f.b}" stroke="var(--blue)" stroke-width="2" stroke-dasharray="4 3"/>`)}
    <div class="note-body">Median ${Math.round(r.median_ms)} ms (dashed line); half of your reactions are between ${Math.round(r.p25_ms)} and ${Math.round(r.p75_ms)} ms. ${r.median_before_ms ? delta(r.median_ms, r.median_before_ms, "ms", -1) + "." : ""}</div>
  </section>`;
}

function distanceCard(ds) {
  return `<section class="card"><div class="card-head"><h2>By flick distance</h2><span class="small muted">Share hit and median time</span></div>
    ${bars(ds.map((d) => [`${d.from_deg}–${d.to_deg >= 180 ? "+" : d.to_deg}°`, d.success, `${ms(d.median_ms)} · ${d.targets}`]), share)}
  </section>`;
}

function trackingCard(t) {
  const W = CHART.w;
  const lead = Math.max(-2, Math.min(2, t.lead_deg));
  return `<section class="card"><div class="card-head"><h2>Tracking</h2><span class="small muted">Sampled 30 times a second</span></div>
    <h3>On target by target speed</h3>${bars(t.by_speed.map((b) => [b.label, b.on_target, `${b.samples}`]), share)}
    <h3>After the target changes direction</h3>${bars(t.after_change.map((b) => [b.label, b.on_target, ""]), share)}
    ${t.stretch_s != null ? `<h3>Unbroken on target</h3><div class="note-body">Half your time on target came in unbroken stretches of ${t.stretch_s.toFixed(1)} s or longer${t.stretch_before_s != null ? ` (${t.stretch_before_s.toFixed(1)} s before)` : ""}. Longer stretches mean steadier tracking: the share of time on target counts many short slips the same as one long one.</div>` : ""}
    ${t.centered_deg != null ? `<h3>Centering</h3><div class="note-body">While on target, your crosshair sits ${t.centered_deg.toFixed(2)}° from the target's center (median)${t.centered_before_deg != null ? `, ${t.centered_before_deg.toFixed(2)}° before` : ""}. Lower is more centered: the share of time on target counts the edge of a target the same as its middle.</div>` : ""}
    <div class="note-body">${t.recovery_s === null ? "Not enough direction changes yet to measure recovery." : t.recovery_s === 0 ? "Direction changes don't cost you time on target." : `Your time on target is back to normal about ${Math.round(t.recovery_s * 1000)} ms after a direction change.`}</div>
    <h3>Ahead or behind</h3>
    ${svg(`<line x1="20" x2="${W - 20}" y1="30" y2="30" stroke="var(--border)" stroke-width="6" stroke-linecap="round"/>
      <line x1="${W / 2}" x2="${W / 2}" y1="16" y2="44" stroke="var(--dim)"/>
      <circle cx="${W / 2 + (lead / 2) * (W / 2 - 30)}" cy="30" r="9" fill="var(--rust)"/>
      <text x="20" y="62" font-size="12" fill="var(--dim)">2° behind</text><text x="${W - 20}" y="62" font-size="12" text-anchor="end" fill="var(--dim)">2° ahead</text>
      <text x="${W / 2}" y="62" font-size="12" text-anchor="middle" fill="var(--dim)">on the target</text>`, 70)}
    <div class="note-body">On average your crosshair is ${Math.abs(t.lead_deg).toFixed(2)}° ${t.lead_deg < 0 ? "behind" : "ahead of"} the target along its motion.</div>
  </section>`;
}

function segmentCard(segs) {
  return `<section class="card"><div class="card-head"><h2>Within a run</h2><span class="small muted">Share hit by 10-second segment</span></div>
    ${bars(segs.filter((s) => s.success !== null).map((s) => [`${s.from_s}–${s.to_s} s`, s.success, ""]), share)}
    <div class="small muted">A drop at the end suggests fatigue; a low start suggests you need a warm-up.</div>
  </section>`;
}
