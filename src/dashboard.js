const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

/* ---------- theme: follow system ---------- */
function applyTheme() {
  const light = matchMedia("(prefers-color-scheme: light)").matches;
  document.documentElement.setAttribute("data-theme", light ? "light" : "dark");
}
applyTheme();
matchMedia("(prefers-color-scheme: light)").addEventListener("change", () => {
  applyTheme();
  if (window._chartData) drawChart(window._chartData);
});

/* ---------- tabs ---------- */
function showTab(t) {
  document.getElementById("view-dashboard").classList.toggle("hidden", t !== "dashboard");
  document.getElementById("view-settings").classList.toggle("hidden", t !== "settings");
  document.getElementById("tab-dashboard").classList.toggle("active", t === "dashboard");
  document.getElementById("tab-settings").classList.toggle("active", t === "settings");
}
document.getElementById("tab-dashboard").onclick = () => showTab("dashboard");
document.getElementById("tab-settings").onclick = () => showTab("settings");

/* ---------- dashboard data ---------- */
async function loadDashboard() {
  const data = await invoke("get_dashboard_data");
  document.getElementById("t-success").textContent = data.today_successful;
  document.getElementById("t-skip").textContent = data.today_skipped;
  document.getElementById("t-missed").textContent = data.today_meeting_missed;
  document.getElementById("t-streak").textContent = data.streak;
  document.getElementById("t-total").textContent = data.all_time_total;
  window._chartData = data.days;
  drawChart(data.days);
}

/* ---------- chart ---------- */
const svg = document.getElementById("chart");
const W = 760, H = 240, padL = 30, padB = 26, padT = 10, padR = 8;
const plotW = W - padL - padR, plotH = H - padB - padT;
const tip = document.getElementById("tip");

function drawChart(days) {
  const c = getComputedStyle(document.documentElement);
  const COL = { s: c.getPropertyValue("--success").trim(), k: c.getPropertyValue("--skip").trim() };
  const maxV = Math.max(1, ...days.map((d) => Math.max(d.successful, d.skipped))) + 1;

  let s = "";
  for (let i = 0; i <= maxV; i += Math.max(1, Math.ceil(maxV / 6))) {
    const y = padT + plotH - (i / maxV) * plotH;
    s += `<line class="gridline" x1="${padL}" y1="${y}" x2="${W - padR}" y2="${y}"/>`;
    s += `<text class="tick" x="${padL - 8}" y="${y + 3}" text-anchor="end">${i}</text>`;
  }
  const groupW = plotW / days.length;
  const barW = Math.min(11, (groupW - 6) / 2);
  days.forEach((d, i) => {
    const gx = padL + i * groupW + groupW / 2;
    [["successful", d.successful, -1, "s"], ["skipped", d.skipped, 1, "k"]].forEach(([key, v, side]) => {
      const h = (v / maxV) * plotH;
      const x = gx + (side < 0 ? -barW - 1 : 1);
      const y = padT + plotH - h;
      s += `<rect class="bar" x="${x}" y="${y}" width="${barW}" height="${h}" rx="4" fill="var(--${key === "successful" ? "success" : "skip"})" data-i="${i}"></rect>`;
    });
    const label = d.date.slice(8); // DD
    s += `<text class="tick" x="${gx}" y="${H - 8}" text-anchor="middle">${label}</text>`;
  });
  svg.innerHTML = s;

  svg.querySelectorAll(".bar").forEach((b) => {
    b.addEventListener("mousemove", (e) => showTip(e, days[+b.dataset.i], COL));
    b.addEventListener("mouseleave", () => (tip.style.opacity = 0));
  });
}

function showTip(e, d, COL) {
  tip.innerHTML = `<div class="tt-d">${d.date}</div>
    <div class="tt-r"><span><span class="sw" style="background:${COL.s}"></span>Successful</span><b>${d.successful}</b></div>
    <div class="tt-r"><span><span class="sw" style="background:${COL.k}"></span>Skipped</span><b>${d.skipped}</b></div>`;
  tip.style.opacity = 1;
  let x = e.clientX + 14;
  if (x + 150 > innerWidth) x = e.clientX - 160;
  tip.style.left = x + "px";
  tip.style.top = e.clientY - 10 + "px";
}

/* ---------- settings ---------- */
let settings = null;

function renderSettings() {
  document.querySelectorAll(".stepper").forEach((st) => {
    st.querySelector(".num").textContent = settings[st.dataset.key];
  });
  document.querySelectorAll(".switch").forEach((sw) => {
    const key = sw.dataset.key;
    let on;
    if (key === "manual_meeting_override") on = settings.manual_meeting_override === true;
    else on = !!settings[key];
    sw.classList.toggle("on", on);
  });
}

async function loadSettings() {
  settings = await invoke("get_settings");
  renderSettings();
}

async function saveSettings() {
  settings = await invoke("set_settings", { new: settings });
  renderSettings();
}

document.querySelectorAll(".stepper button").forEach((btn) => {
  btn.onclick = () => {
    const st = btn.closest(".stepper");
    const key = st.dataset.key;
    const min = +st.dataset.min, max = +st.dataset.max;
    settings[key] = Math.max(min, Math.min(max, settings[key] + +btn.dataset.step));
    saveSettings();
  };
});

document.querySelectorAll(".switch").forEach((sw) => {
  sw.onclick = async () => {
    const key = sw.dataset.key;
    if (key === "manual_meeting_override") {
      const on = settings.manual_meeting_override === true;
      settings = await invoke("set_settings", {
        new: { ...settings, manual_meeting_override: on ? null : true },
      });
      renderSettings();
    } else {
      settings[key] = !settings[key];
      saveSettings();
    }
  };
});

/* ---------- live meeting indicator ---------- */
async function pollMeeting() {
  try {
    const active = await invoke("get_meeting_active");
    const el = document.getElementById("meeting-live");
    el.classList.toggle("active", active);
    document.getElementById("meeting-live-text").textContent = active ? "meeting detected now" : "no meeting detected";
  } catch (_) {}
}

/* ---------- status line + live timer ---------- */
function fmtRemaining(secs) {
  if (secs >= 60) return `${Math.ceil(secs / 60)}m`;
  return `${secs}s`;
}
function fmtClock(secs) {
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
}

function renderStatus(state) {
  const el = document.getElementById("status");
  const txt = document.getElementById("status-text");
  const btn = document.getElementById("pause-btn");
  el.classList.toggle("paused", state.phase === "paused");
  if (state.phase === "paused") {
    txt.textContent = "Paused";
  } else if (state.phase === "break") {
    txt.textContent = `On a break · ${fmtRemaining(state.remaining_secs)} left`;
  } else {
    txt.textContent = `Running · next break in ${fmtRemaining(state.remaining_secs)}`;
  }
  if (btn) btn.textContent = state.phase === "paused" ? "Resume" : "Pause";
}

function renderTimer(state) {
  const card = document.getElementById("timer-card");
  card.classList.toggle("paused", state.phase === "paused");
  card.classList.toggle("break", state.phase === "break");
  document.getElementById("tc-time").textContent = fmtClock(Math.max(0, state.remaining_secs));
  const phase = document.getElementById("tc-phase");
  const sub = document.getElementById("tc-sub");
  if (state.phase === "paused") {
    phase.textContent = "Paused";
    sub.textContent = "timer is on hold";
  } else if (state.phase === "break") {
    phase.textContent = "On a break";
    sub.textContent = "stand up and move";
  } else {
    phase.textContent = "Focusing";
    sub.textContent = "next break on the clock";
  }
  document.getElementById("tc-pause").textContent = state.phase === "paused" ? "Resume" : "Pause";
}

function onState(state) {
  renderStatus(state);
  renderTimer(state);
}

async function togglePause() {
  onState(await invoke("toggle_pause"));
}
document.getElementById("pause-btn").onclick = togglePause;
document.getElementById("tc-pause").onclick = togglePause;

listen("engine-state", (e) => onState(e.payload));

/* ---------- init ---------- */
loadDashboard();
loadSettings();
invoke("get_engine_state").then(onState).catch(() => {});
pollMeeting();
setInterval(pollMeeting, 5000);
setInterval(loadDashboard, 30000);
