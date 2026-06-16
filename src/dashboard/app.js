const state = { sessions: [], reconnect: 0, source: null };
const delays = [1000, 2000, 5000, 10000];
const byId = (id) => document.getElementById(id);

function formatDuration(milliseconds) {
  const seconds = Math.max(0, Math.floor(milliseconds / 1000));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = seconds % 60;
  return hours ? `${hours}h ${minutes}m` : minutes ? `${minutes}m ${rest}s` : `${rest}s`;
}

function formatBytes(bytes) {
  if (bytes === null || bytes === undefined) return "Unavailable";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) { value /= 1024; unit += 1; }
  return `${value.toFixed(unit ? 1 : 0)} ${units[unit]}`;
}

function metric(metric, formatter) {
  if (!metric || metric.quality === "unavailable") return "Unavailable";
  return `${formatter(metric.value)} · ${metric.quality}`;
}

function escapeText(value) {
  const node = document.createElement("span");
  node.textContent = value;
  return node.innerHTML;
}

function escapeAttribute(value) {
  return escapeText(value).replaceAll('"', "&quot;").replaceAll("'", "&#39;");
}

function renderLive() {
  const grid = byId("live-grid");
  byId("active-count").textContent = state.sessions.length;
  grid.innerHTML = state.sessions.map((session) => `
    <article class="session-card">
      <div class="card-top">
        <div><div class="agent">${escapeText(session.agent_id)}</div><div class="adapter">${escapeText(session.adapter_id)} adapter</div></div>
        <div class="state">${escapeText(session.state.replaceAll("_", " "))}</div>
      </div>
      <div class="duration" data-started="${session.started_at_ms}">${formatDuration(Date.now() - session.started_at_ms)}</div>
      <div class="metrics">
        <div class="metric"><span>CPU</span><strong>${metric(session.cpu_percent, (value) => `${value.toFixed(1)}%`)}</strong></div>
        <div class="metric"><span>Memory</span><strong>${metric(session.memory_bytes, formatBytes)}</strong></div>
        <div class="metric"><span>Context</span><strong>${metric(session.context_pressure, (value) => `${value.toFixed(1)}%`)}</strong></div>
      </div>
    </article>`).join("");
}

function connect() {
  if (state.source) state.source.close();
  const source = new EventSource("/v1/sessions/stream");
  state.source = source;
  source.addEventListener("sessions", (event) => {
    state.sessions = JSON.parse(event.data);
    state.reconnect = 0;
    setConnection("online", "Live");
    renderLive();
  });
  source.onerror = () => {
    source.close();
    setConnection("offline", "Reconnecting");
    const delay = delays[Math.min(state.reconnect, delays.length - 1)];
    state.reconnect += 1;
    window.setTimeout(connect, delay);
  };
}

function setConnection(status, label) {
  byId("connection").dataset.state = status;
  byId("connection-label").textContent = label;
}

function renderStats(stats) {
  const completed = stats.states.completed || 0;
  byId("stats-grid").innerHTML = [
    ["Sessions", stats.session_count],
    ["Completed", completed],
    ["Average duration", stats.average_duration_ms === null ? "Unavailable" : formatDuration(stats.average_duration_ms)],
    ["Peak memory", formatBytes(stats.peak_memory_bytes)],
  ].map(([label, value]) => `<div class="stat-card"><span>${label}</span><strong>${value}</strong></div>`).join("");
}

function renderHistory(page) {
  byId("history-body").innerHTML = page.sessions.map((session) => `
    <tr>
      <td>${escapeText(session.agent_id)}</td>
      <td><span class="result ${session.final_state}">${escapeText(session.final_state)}</span></td>
      <td>${new Date(session.started_at_ms).toLocaleString()}</td>
      <td>${formatDuration(session.duration_ms)}</td>
      <td>${session.peak_cpu_percent === null ? "Unavailable" : `${session.peak_cpu_percent.toFixed(1)}%`}</td>
      <td>${formatBytes(session.peak_memory_bytes)}</td>
      <td>${metric(session.context_peak, (value) => `${value.toFixed(1)}%`)}</td>
      <td><button class="row-delete" data-delete="${session.session_id}" aria-label="Delete ${escapeAttribute(session.agent_id)} session">Delete</button></td>
    </tr>`).join("");
  document.querySelectorAll("[data-delete]").forEach((button) => {
    button.addEventListener("click", () => confirmDelete(button.dataset.delete));
  });
}

async function loadHistory() {
  try {
    const [pageResponse, statsResponse] = await Promise.all([fetch("/v1/history?limit=100&offset=0"), fetch("/v1/history/stats")]);
    if (!pageResponse.ok || !statsResponse.ok) throw new Error("History unavailable");
    renderHistory(await pageResponse.json());
    renderStats(await statsResponse.json());
  } catch (error) { showStatus(error.message); }
}

function confirmAction(title, message, action) {
  const dialog = byId("confirm-dialog");
  byId("confirm-title").textContent = title;
  byId("confirm-message").textContent = message;
  dialog.showModal();
  dialog.addEventListener("close", async function handler() {
    dialog.removeEventListener("close", handler);
    if (dialog.returnValue === "confirm") await action();
  });
}

function confirmDelete(sessionId) {
  confirmAction("Delete session?", "This removes one local summary permanently.", async () => {
    const response = await fetch(`/v1/history/${sessionId}`, { method: "DELETE" });
    showStatus(response.ok ? "Session deleted" : "Could not delete session");
    if (response.ok) loadHistory();
  });
}

function confirmPurge() {
  confirmAction("Purge all history?", "This permanently deletes every finalized local summary. Active sessions remain running.", async () => {
    const response = await fetch("/v1/history", { method: "DELETE" });
    showStatus(response.ok ? "History purged" : "Could not purge history");
    if (response.ok) loadHistory();
  });
}

function showStatus(message) {
  const status = byId("status");
  status.textContent = message;
  status.classList.add("visible");
  window.setTimeout(() => status.classList.remove("visible"), 2600);
}

document.querySelectorAll("[data-view]").forEach((button) => {
  button.addEventListener("click", () => {
    document.querySelectorAll("[data-view]").forEach((item) => { item.classList.toggle("active", item === button); item.setAttribute("aria-pressed", item === button); });
    document.querySelectorAll(".view").forEach((view) => { const active = view.id === `${button.dataset.view}-view`; view.hidden = !active; view.classList.toggle("active", active); });
    if (button.dataset.view === "history") loadHistory();
  });
});

byId("purge-button").addEventListener("click", confirmPurge);
window.setInterval(renderLive, 1000);
connect();
