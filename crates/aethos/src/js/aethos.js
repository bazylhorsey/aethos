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

      const resolve = this.pendingRefs.get(msgRef);
      if (resolve) { resolve(response); this.pendingRefs.delete(msgRef); }
    }

    if (event === "diff") {
      this._applyDiff(payload);
    }

    // ── Navigation ───────────────────────────────────────────────────────────
    if (event === "phx_navigate") {
      const url = payload.to;
      if (url) {
        window.history.pushState({}, "", url);
        // Remount: disconnect and reconnect at the new URL
        this.disconnect();
        setTimeout(() => this.connect(), 50);
      }
    }

    if (event === "phx_patch") {
      const url = payload.to;
      if (url) {
        window.history.pushState({}, "", url);
        // Stay connected; server will push a diff with new state
      }
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
    // rendered = {"0": html, "s": [...], "e": [{event, payload}]}
    if (rendered["0"] !== undefined) {
      _destroyHooks(this.root);
      this.root.innerHTML = rendered["0"];
      this._bindEvents();
      _mountHooks(this.root, this);
    }
    if (rendered["e"]) this._applyEvents(rendered["e"]);
  }

  _applyDiff(diff) {
    if (diff["0"] !== undefined) {
      _destroyHooks(this.root);
      this.root.innerHTML = diff["0"];
      this._bindEvents();
      _mountHooks(this.root, this);
      _updateHooks(this.root);
    }
    if (diff.streams) {
      this._applyStreams(diff.streams);
    }
    if (diff["e"]) this._applyEvents(diff["e"]);
  }

  // ── Server-pushed events ───────────────────────────────────────────────────

  _applyEvents(events) {
    for (const ev of events) {
      if (ev.event === "put-flash") {
        const { key, msg } = ev.payload;
        document.dispatchEvent(new CustomEvent("phx:flash", { detail: { key, msg } }));
      } else {
        // Dispatch to any phx-hook that registered handleEvent
        this.root.querySelectorAll("[phx-hook]").forEach(el => {
          if (el._aethosHook?.handleEvent) el._aethosHook.handleEvent(ev.event, ev.payload);
        });
      }
    }
  }

  // ── Streams ────────────────────────────────────────────────────────────────

  _applyStreams(ops) {
    for (const op of ops) {
      const container = this.root.querySelector(`[phx-update="stream"][data-stream="${op.name}"]`);
      if (!container) continue;

      if (op.op === "reset") {
        container.innerHTML = "";
      } else if (op.op === "delete") {
        const el = container.querySelector(`[data-stream-id="${op.id}"]`);
        if (el) el.remove();
      } else if (op.op === "insert") {
        let existing = container.querySelector(`[data-stream-id="${op.id}"]`);
        if (existing) {
          // Update in place
          const tmp = document.createElement("div");
          tmp.innerHTML = typeof op.item === "string" ? op.item : JSON.stringify(op.item);
          const newEl = tmp.firstElementChild;
          if (newEl) existing.replaceWith(newEl);
        } else {
          // Append
          const tmp = document.createElement("div");
          tmp.innerHTML = typeof op.item === "string" ? op.item : JSON.stringify(op.item);
          const newEl = tmp.firstElementChild;
          if (newEl) container.appendChild(newEl);
        }
      }
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
    const csrfToken = document.querySelector("meta[name='csrf-token']")?.content || "";
    this._send(this.joinRef, ref, this.topic, "event", { type, event, value, csrf_token: csrfToken });
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

// ── Hooks registry ─────────────────────────────────────────────────────────────

/**
 * Register client-side hooks for use with phx-hook="HookName".
 *
 * Each hook object may define:
 *   - mounted()     called after the element is added to the DOM
 *   - updated()     called after the element's DOM is patched
 *   - destroyed()   called before the element is removed from the DOM
 *   - handleEvent(event, payload)  called when the server pushes an event
 *
 * The hook's `this` context exposes:
 *   - this.el       the DOM element
 *   - this.pushEvent(event, payload)  send an event to the server
 *
 * Example:
 * ```js
 * AethosHooks.Chart = {
 *   mounted() { this._chart = new Chart(this.el, ...); },
 *   updated() { this._chart.update(...); },
 *   destroyed() { this._chart.destroy(); }
 * };
 * ```
 */
window.AethosHooks = {};

function _mountHooks(root, lv) {
  root.querySelectorAll("[phx-hook]").forEach(el => {
    const name = el.getAttribute("phx-hook");
    const hookDef = window.AethosHooks[name];
    if (!hookDef) {
      console.warn(`[aethos] phx-hook="${name}" not found in AethosHooks`);
      return;
    }
    const hook = Object.create(hookDef);
    hook.el = el;
    hook.pushEvent = (event, payload = {}) => lv._pushEvent("hook", event, payload);
    el._aethosHook = hook;
    if (hook.mounted) hook.mounted();
  });
}

function _updateHooks(root) {
  root.querySelectorAll("[phx-hook]").forEach(el => {
    if (el._aethosHook?.updated) el._aethosHook.updated();
  });
}

function _destroyHooks(root) {
  root.querySelectorAll("[phx-hook]").forEach(el => {
    if (el._aethosHook?.destroyed) el._aethosHook.destroyed();
  });
}

// Export for manual usage
window.AethosLive = AethosLive;
