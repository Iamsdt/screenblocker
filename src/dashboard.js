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

/* ---------- status line ---------- */
function fmtRemaining(secs) {
  if (secs >= 60) return `${Math.ceil(secs / 60)}m`;
  return `${secs}s`;
}
function renderStatus(state) {
  const el = document.getElementById("status");
  const txt = document.getElementById("status-text");
  if (state.phase === "paused") {
    el.classList.add("paused");
    txt.textContent = "Paused";
  } else if (state.phase === "break") {
    el.classList.remove("paused");
    txt.textContent = `On a break · ${fmtRemaining(state.remaining_secs)} left`;
  } else {
    el.classList.remove("paused");
    txt.textContent = `Running · next break in ${fmtRemaining(state.remaining_secs)}`;
  }
}

listen("engine-state", (e) => renderStatus(e.payload));

/* ---------- init ---------- */
loadDashboard();
loadSettings();
invoke("get_engine_state").then(renderStatus).catch(() => {});
pollMeeting();
setInterval(pollMeeting, 5000);
setInterval(loadDashboard, 30000);
