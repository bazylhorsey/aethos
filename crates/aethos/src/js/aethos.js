/**
 * aethos.js — LiveView client for the Aethos framework.
 *
 * Connects to the server over WebSocket, sends the Phoenix wire protocol
 * (JSON arrays), and patches the DOM when diffs arrive.
 *
 * Wire protocol (Phoenix-compatible):
 *   Client→Server: [join_ref, msg_ref, topic, event, payload]
 *   Server→Client: [join_ref, msg_ref, topic, event, payload]
 */

const HEARTBEAT_INTERVAL = 30_000; // ms

class AethosLive {
  constructor(rootEl) {
    this.root = rootEl;
    this.topic = rootEl.dataset.topic || "lv:" + (rootEl.id || "root");
    this.joinRef = String(Math.random()).slice(2);
    this.msgRef = 0;
    this.ws = null;
    this.heartbeatTimer = null;
    this.pendingRefs = new Map(); // ref → resolve/reject
  }

  // ── Lifecycle ──────────────────────────────────────────────────────────────

  connect() {
    const protocol = location.protocol === "https:" ? "wss" : "ws";
    const url = `${protocol}://${location.host}${location.pathname}`;
    this.ws = new WebSocket(url);
    this.ws.onopen    = () => this._onOpen();
    this.ws.onmessage = (ev) => this._onMessage(ev);
    this.ws.onclose   = () => this._onClose();
    this.ws.onerror   = (ev) => console.error("[aethos] ws error", ev);
  }

  disconnect() {
    clearInterval(this.heartbeatTimer);
    this.ws && this.ws.close();
  }

  // ── Internal ───────────────────────────────────────────────────────────────

  _onOpen() {
    this._send(this.joinRef, this._nextRef(), this.topic, "phx_join", {});
    this.heartbeatTimer = setInterval(() => this._heartbeat(), HEARTBEAT_INTERVAL);
  }

  _onMessage(ev) {
    let msg;
    try { msg = JSON.parse(ev.data); } catch { return; }

    const [joinRef, msgRef, topic, event, payload] = msg;

    if (event === "phx_reply") {
      const { status, response } = payload;
      if (status !== "ok") return;

      if (response.rendered) {
        this._applyRendered(response.rendered);
      } else if (response.diff) {
        this._applyDiff(response.diff);
      }

      // Resolve pending promise if any
      const resolve = this.pendingRefs.get(msgRef);
      if (resolve) { resolve(response); this.pendingRefs.delete(msgRef); }
    }

    if (event === "diff") {
      this._applyDiff(payload);
    }
  }

  _onClose() {
    clearInterval(this.heartbeatTimer);
    // Reconnect after 2 seconds
    setTimeout(() => this.connect(), 2000);
  }

  _heartbeat() {
    this._send(null, this._nextRef(), "phoenix", "heartbeat", {});
  }

  _nextRef() {
    return String(++this.msgRef);
  }

  _send(joinRef, msgRef, topic, event, payload) {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify([joinRef, msgRef, topic, event, payload]));
    }
  }

  // ── DOM patching ───────────────────────────────────────────────────────────

  _applyRendered(rendered) {
    // rendered = {"0": html, "s": [...]}
    if (rendered["0"] !== undefined) {
      this.root.innerHTML = rendered["0"];
      this._bindEvents();
    }
  }

  _applyDiff(diff) {
    if (diff["0"] !== undefined) {
      this.root.innerHTML = diff["0"];
      this._bindEvents();
    }
  }

  // ── Event binding ──────────────────────────────────────────────────────────

  _bindEvents() {
    this.root.querySelectorAll("[phx-click]").forEach(el => {
      el.addEventListener("click", (ev) => {
        ev.preventDefault();
        this._pushEvent("click", el.getAttribute("phx-click"), this._valueOf(el));
      });
    });

    this.root.querySelectorAll("[phx-submit]").forEach(el => {
      el.addEventListener("submit", (ev) => {
        ev.preventDefault();
        const form = ev.target;
        const data = Object.fromEntries(new FormData(form));
        this._pushEvent("submit", form.getAttribute("phx-submit"), data);
      });
    });

    this.root.querySelectorAll("[phx-change]").forEach(el => {
      el.addEventListener("input", (ev) => {
        this._pushEvent("change", ev.target.getAttribute("phx-change"), { value: ev.target.value });
      });
    });

    this.root.querySelectorAll("[phx-blur]").forEach(el => {
      el.addEventListener("blur", (ev) => {
        this._pushEvent("blur", ev.target.getAttribute("phx-blur"), { value: ev.target.value });
      });
    });

    this.root.querySelectorAll("[phx-keydown]").forEach(el => {
      el.addEventListener("keydown", (ev) => {
        this._pushEvent("keydown", el.getAttribute("phx-keydown"), { key: ev.key });
      });
    });

    this.root.querySelectorAll("[phx-keyup]").forEach(el => {
      el.addEventListener("keyup", (ev) => {
        this._pushEvent("keyup", el.getAttribute("phx-keyup"), { key: ev.key });
      });
    });
  }

  _valueOf(el) {
    return el.dataset.value !== undefined ? { value: el.dataset.value }
         : el.value     !== undefined ? { value: el.value }
         : {};
  }

  _pushEvent(type, event, value) {
    const ref = this._nextRef();
    this._send(this.joinRef, ref, this.topic, "event", { type, event, value });
  }
}

// ── Auto-boot ──────────────────────────────────────────────────────────────────

document.addEventListener("DOMContentLoaded", () => {
  document.querySelectorAll("[data-phx-live]").forEach(el => {
    const lv = new AethosLive(el);
    lv.connect();
    el._aethosLive = lv;
  });
});

// Export for manual usage
window.AethosLive = AethosLive;
