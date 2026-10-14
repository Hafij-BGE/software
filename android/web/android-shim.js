// Craft Suite for Android, loaded before each web app's own scripts (packaging/build-apk.sh adds
// it to their index.html):
//
// - Saving: hands the apps' downloads (how they save a file) to the app, which asks where to save
//   it. A WebView doesn't download blob: links itself.
// - Two fingers: dragging with two fingers scrolls (the apps get it as a mouse wheel), pinching
//   zooms; one finger works as before (select, move, tap).
// - A stylus: writes and selects at once, without the hold a finger has; the hand resting on the
//   screen meanwhile is ignored (palm rejection).
// - Opening: a file opened with Craft Suite from another app (a file manager, mail) arrives in the
//   app as if it had been dropped on it.
// - Read-only (?readonly=1: Craft Suite Viewer, or the launcher's Read-only switch): WordCraft opens
//   the file in Read Mode and DeckCraft as a slide show; in all three typing, the on-screen keyboard
//   and saving are off, and an Edit button opens the file again, editable.
(function () {
  "use strict";
  if (window.__craftSuiteShim || !window.CraftSuiteAndroid) return;
  window.__craftSuiteShim = true;
  var bridge = window.CraftSuiteAndroid;
  var readOnly = /[?&]readonly=1(&|$)/.test(location.search);

  // ---- saving ---------------------------------------------------------------------------------

  // The blobs behind object URLs, so a save still works if the app revokes the URL early.
  var blobs = new Map();
  var create = URL.createObjectURL;
  var revoke = URL.revokeObjectURL;
  URL.createObjectURL = function (obj) {
    var url = create.call(URL, obj);
    if (obj instanceof Blob) blobs.set(url, obj);
    return url;
  };
  URL.revokeObjectURL = function (url) {
    setTimeout(function () { blobs.delete(url); }, 60000);
    return revoke.call(URL, url);
  };

  function send(name, blob) {
    if (readOnly) {
      if (bridge.toast) bridge.toast("Read-only: tap Edit to make changes and save");
      return;
    }
    var reader = new FileReader();
    reader.onload = function () {
      var data = String(reader.result);
      bridge.save(name, blob.type || "", data.substring(data.indexOf(",") + 1));
    };
    reader.onerror = function () { console.error("Craft Suite: couldn't read " + name); };
    reader.readAsDataURL(blob);
  }

  var click = HTMLAnchorElement.prototype.click;
  HTMLAnchorElement.prototype.click = function () {
    var href = this.href || "";
    if (this.hasAttribute("download") && (href.indexOf("blob:") === 0 || href.indexOf("data:") === 0)) {
      var name = this.getAttribute("download") || "download";
      var blob = blobs.get(href);
      if (blob) {
        send(name, blob);
      } else {
        fetch(href).then(function (r) { return r.blob(); }).then(function (b) { send(name, b); })
          .catch(function (e) { console.error("Craft Suite: couldn't save " + name, e); });
      }
      return;
    }
    return click.call(this);
  };

  // ---- touch: two fingers scroll, pinch zooms, a stylus writes ---------------------------------

  // A finger reaches the app twice, as a pointer event and as a touch event. The first finger is
  // held back this long: if a second one lands meanwhile, it's a two-finger gesture and the app
  // never sees a press (no stray selection or tap); otherwise everything held is passed on, in
  // order. A stylus is never held back (ink starts at once), and while it's down, a hand resting
  // on the screen is ignored.
  var HOLD_MS = 90;
  var replaying = false;   // our own re-sent events pass through
  var held = null;         // {pointer, start, moves: [], target, timer}
  var gesture = null;      // {x, y, dist, startDist, zooming, target}
  var fingers = new Set(); // touch pointers down
  var swallowing = false;  // a gesture ran: its pointer events stay out until every finger is up
  var pressed = false;     // the app has a one-finger press it hasn't seen end
  var penDown = false;     // a stylus touches the screen (its pointer event comes first)
  var stylus = null;       // {id, target}: the stylus's touch, the only one the app sees
  var palm = false;        // the stylus lifted with the hand still down: ignore it until it lifts

  function canvasOf(e) {
    var t = e.target;
    return t && t.tagName === "CANVAS" ? t : null;
  }

  function mid(touches) {
    var a = touches[0], b = touches[1];
    return { x: (a.clientX + b.clientX) / 2, y: (a.clientY + b.clientY) / 2,
      dist: Math.hypot(a.clientX - b.clientX, a.clientY - b.clientY) };
  }

  function list(l) { return Array.prototype.slice.call(l); }

  function find(l, id) {
    for (var i = 0; i < l.length; i++) if (l[i].identifier === id) return l[i];
    return null;
  }

  function touchEvent(type, e, touches, changed) {
    return new TouchEvent(type, { bubbles: true, cancelable: true, composed: true,
      touches: touches, targetTouches: touches, changedTouches: changed,
      ctrlKey: e.ctrlKey, shiftKey: e.shiftKey, altKey: e.altKey, metaKey: e.metaKey });
  }

  function copyTouch(e) {
    return touchEvent(e.type, e, list(e.touches), list(e.changedTouches));
  }

  function copyPointer(e) {
    return new PointerEvent(e.type, { bubbles: true, cancelable: true, composed: true, pointerId: e.pointerId,
      pointerType: e.pointerType, isPrimary: e.isPrimary, clientX: e.clientX, clientY: e.clientY, screenX: e.screenX,
      screenY: e.screenY, button: e.button, buttons: e.buttons, pressure: e.pressure, width: e.width, height: e.height,
      ctrlKey: e.ctrlKey, shiftKey: e.shiftKey, altKey: e.altKey, metaKey: e.metaKey });
  }

  function replay(ev, target) {
    replaying = true;
    try { target.dispatchEvent(ev); } finally { replaying = false; }
  }

  function release() {
    if (!held) return;
    clearTimeout(held.timer);
    var h = held;
    held = null;
    if (h.pointer) replay(copyPointer(h.pointer), h.target);
    if (h.start) replay(copyTouch(h.start), h.target);
    pressed = true;
    // Every move, so a stroke drawn with a finger keeps its first points.
    h.moves.forEach(function (m) { replay(copyTouch(m), h.target); });
  }

  function hold(target) {
    if (!held) held = { pointer: null, start: null, moves: [], target: target, timer: setTimeout(release, HOLD_MS) };
    return held;
  }

  // Ends the app's one-finger press (a finger it saw before a gesture or the stylus began).
  function endPress(touch, target) {
    if (!pressed || !touch) return;
    replay(touchEvent("touchend", {}, [], [touch]), target);
    pressed = false;
  }

  function wheel(target, x, y, dx, dy, ctrl) {
    // The app scrolls what's under the pointer: put it between the fingers first.
    target.dispatchEvent(new MouseEvent("mousemove", { bubbles: true, cancelable: true, clientX: x, clientY: y }));
    target.dispatchEvent(new WheelEvent("wheel", { bubbles: true, cancelable: true, clientX: x, clientY: y,
      deltaX: dx, deltaY: dy, deltaMode: 0, ctrlKey: !!ctrl }));
  }

  function swallow(e) {
    e.stopImmediatePropagation();
    if (e.cancelable) e.preventDefault();
  }

  // Pointer events (they come first). The app takes fingers and the stylus from touch events;
  // their pointer events would press a second time.
  window.addEventListener("pointerdown", function (e) {
    if (replaying) return;
    if (e.pointerType === "pen") {
      penDown = true;
      e.stopImmediatePropagation();
      return;
    }
    if (e.pointerType !== "touch") return;
    fingers.add(e.pointerId);
    if (gesture || swallowing || stylus || palm || fingers.size > 1) { swallow(e); return; }
    var target = canvasOf(e);
    if (target) {
      swallow(e);
      hold(target).pointer = e;
    }
  }, { capture: true, passive: false });

  function pointerEnd(e) {
    if (replaying) return;
    if (e.pointerType === "pen") {
      penDown = false;
      e.stopImmediatePropagation();
      return;
    }
    if (e.pointerType !== "touch") return;
    fingers.delete(e.pointerId);
    if (gesture || swallowing || stylus || palm) {
      swallow(e);
      if (fingers.size === 0 && !gesture) swallowing = false;
      return;
    }
    // A quick tap: the press first, then this release.
    if (held) release();
  }
  window.addEventListener("pointerup", pointerEnd, { capture: true, passive: false });
  window.addEventListener("pointercancel", pointerEnd, { capture: true, passive: false });

  window.addEventListener("touchstart", function (e) {
    if (replaying) return;
    if (stylus || palm) { swallow(e); return; }
    var target = canvasOf(e);
    if (penDown) {
      // The stylus: the touch that just landed. Any finger already down is a resting hand.
      var pen = e.changedTouches[e.changedTouches.length - 1];
      var t = target || (held && held.target) || (gesture && gesture.target);
      if (!t || !pen) return;
      var hand = list(e.touches).filter(function (x) { return x.identifier !== pen.identifier; });
      if (held) { clearTimeout(held.timer); held = null; }
      gesture = null;
      endPress(hand[0], t);
      stylus = { id: pen.identifier, target: t };
      if (hand.length) {
        swallow(e);
        replay(touchEvent("touchstart", e, [pen], [pen]), t);
      }
      return;
    }
    if (gesture) { swallow(e); return; }
    if (e.touches.length >= 2 && (target || held)) {
      var g = (held && held.target) || target;
      if (held) {
        clearTimeout(held.timer);
        held = null;
      } else {
        // The second finger came late: end the app's one-finger press first.
        endPress(e.touches[0], g);
      }
      var m = mid(e.touches);
      gesture = { x: m.x, y: m.y, dist: m.dist, startDist: m.dist, zooming: false, target: g };
      swallowing = true;
      swallow(e);
      return;
    }
    if (target && e.touches.length === 1) {
      swallow(e);
      hold(target).start = e;
    }
  }, { capture: true, passive: false });

  window.addEventListener("touchmove", function (e) {
    if (replaying) return;
    if (stylus) {
      var st = find(e.touches, stylus.id);
      if (e.touches.length === 1 && st) return; // the stylus alone: as it is
      swallow(e);
      if (st && find(e.changedTouches, stylus.id)) replay(touchEvent("touchmove", e, [st], [st]), stylus.target);
      return;
    }
    if (palm) { swallow(e); return; }
    if (gesture) {
      swallow(e);
      if (e.touches.length < 2) return;
      var m = mid(e.touches);
      var dx = m.x - gesture.x, dy = m.y - gesture.y;
      // A pinch once the fingers' distance changed by 10%; a pan otherwise.
      if (!gesture.zooming && Math.abs(m.dist - gesture.startDist) > gesture.startDist * 0.1) gesture.zooming = true;
      if (gesture.zooming && gesture.dist > 0 && m.dist > 0) {
        var ratio = m.dist / gesture.dist;
        // eframe: zoom = exp(-0.01 × deltaY) for a ctrl+wheel (a trackpad pinch).
        if (Math.abs(ratio - 1) > 0.002) wheel(gesture.target, m.x, m.y, 0, -Math.log(ratio) / 0.01, true);
      }
      if (dx || dy) wheel(gesture.target, m.x, m.y, -dx, -dy, false);
      gesture.x = m.x; gesture.y = m.y; gesture.dist = m.dist;
      return;
    }
    if (held) {
      swallow(e);
      held.moves.push(e);
    }
  }, { capture: true, passive: false });

  function touchEnd(e) {
    if (replaying) return;
    if (stylus) {
      var st = find(e.changedTouches, stylus.id);
      if (!st) { swallow(e); return; } // the hand moved off: nothing for the app
      var target = stylus.target;
      stylus = null;
      if (e.touches.length > 0) {
        // The stylus lifted, the hand is still down.
        swallow(e);
        replay(touchEvent(e.type, e, [], [st]), target);
        palm = true;
      }
      return;
    }
    if (palm) {
      swallow(e);
      if (e.touches.length === 0) palm = false;
      return;
    }
    if (gesture) {
      swallow(e);
      if (e.touches.length === 0) {
        // No pointer left hovering where the fingers were.
        gesture.target.dispatchEvent(new MouseEvent("mouseleave", { bubbles: false }));
        gesture = null;
        if (fingers.size === 0) swallowing = false;
      }
      return;
    }
    if (held) release();
    if (e.touches.length === 0) pressed = false;
  }
  window.addEventListener("touchend", touchEnd, { capture: true, passive: false });
  window.addEventListener("touchcancel", touchEnd, { capture: true, passive: false });

  // ---- read-only -------------------------------------------------------------------------------

  var APP = (location.pathname.match(/\/(word|deck|grid)\//) || [])[1];

  // The apps' own commands, through android/patches/android-commands.patch.
  function cmd(id, params, afterOpen) {
    var b = window.wasmBindings || {};
    var f = afterOpen ? b.craft_suite_after_open : b.craft_suite_command;
    if (typeof f !== "function") return false;
    f(id, JSON.stringify(params || {}));
    return true;
  }

  // Only steps that change how the file is shown, never the file (a changed file would ask
  // "Leave without saving?"): typing and saving are off in read-only mode anyway.
  function readOnlySteps(afterOpen) {
    if (APP === "word") {
      cmd("view.readMode", {}, afterOpen);
    } else if (APP === "deck") {
      cmd("show.fromStart", {}, afterOpen);
    }
  }

  function key(name) {
    var init = { key: name, code: name, bubbles: true, cancelable: true };
    replaying = true;
    try {
      document.dispatchEvent(new KeyboardEvent("keydown", init));
      document.dispatchEvent(new KeyboardEvent("keyup", init));
    } finally { replaying = false; }
  }

  function editSteps() {
    if (APP === "word" || APP === "deck") key("Escape"); // out of Read Mode / the slide show
  }

  // Keys that look or move, never change: arrows, paging, Escape, copy, select all, find, zoom.
  function harmless(e) {
    if (["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "PageUp", "PageDown", "Home", "End", "Escape", "Tab"].indexOf(e.key) >= 0) return true;
    return (e.ctrlKey || e.metaKey) && !e.altKey && ["c", "a", "f", "+", "-", "=", "0"].indexOf(String(e.key).toLowerCase()) >= 0;
  }

  function guard(e) {
    if (!readOnly || replaying) return;
    if ((e.type === "keydown" || e.type === "keyup") && harmless(e)) return;
    swallow(e);
  }
  ["keydown", "keypress", "keyup", "beforeinput", "input", "paste", "cut", "compositionstart", "compositionupdate", "compositionend"]
    .forEach(function (t) { window.addEventListener(t, guard, { capture: true, passive: false }); });

  // No on-screen keyboard while reading: eframe's hidden text field asks for none.
  function quietKeyboard() {
    var input = document.querySelector("input[type=text]");
    if (!input) return;
    if (readOnly) {
      input.setAttribute("inputmode", "none");
      input.readOnly = true;
    } else {
      input.removeAttribute("inputmode");
      input.readOnly = false;
    }
  }

  function banner() {
    if (!readOnly || document.getElementById("craft-suite-ro")) return;
    var bar = document.createElement("div");
    bar.id = "craft-suite-ro";
    bar.style.cssText = "position:fixed;left:16px;bottom:44px;z-index:2147483647;display:flex;align-items:center;gap:10px;" +
      "padding:6px 6px 6px 14px;border-radius:999px;background:rgba(32,33,36,.92);color:#fff;" +
      "font:14px/1.2 system-ui,Roboto,sans-serif;box-shadow:0 2px 10px rgba(0,0,0,.25);user-select:none";
    var label = document.createElement("span");
    label.textContent = "Read-only";
    var edit = document.createElement("button");
    edit.type = "button";
    edit.textContent = "Edit";
    edit.style.cssText = "border:none;border-radius:999px;padding:7px 14px;background:#7d97ff;color:#10131a;font:600 14px system-ui,Roboto,sans-serif";
    edit.addEventListener("click", function (e) {
      e.stopPropagation();
      // The same file again, for editing (the app keeps it): a fresh start, whatever the read-only
      // steps did. Without it (an older app), undo the steps here.
      if (bridge.reopen && bridge.reopen()) {
        location.replace(location.pathname);
        return;
      }
      readOnly = false;
      quietKeyboard();
      editSteps();
      bar.remove();
      try { history.replaceState(null, "", location.pathname); } catch (err) { /* stays */ }
      if (bridge.toast) bridge.toast("Editing: changes can be saved");
    });
    ["touchstart", "touchend", "pointerdown", "pointerup"].forEach(function (t) {
      bar.addEventListener(t, function (e) { e.stopPropagation(); });
    });
    bar.appendChild(label);
    bar.appendChild(edit);
    document.body.appendChild(bar);
  }

  if (readOnly) {
    var tries = 0;
    var timer = setInterval(function () {
      quietKeyboard();
      if (document.body) banner();
      if (++tries > 100 || document.querySelector("input[type=text]")) clearInterval(timer);
    }, 100);
  }

  // ---- a file opened from another app ------------------------------------------------------------

  function b64ToBytes(b64) {
    var bin = atob(b64);
    var bytes = new Uint8Array(bin.length);
    for (var i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    return bytes;
  }

  function deliver(tries) {
    var canvas = document.querySelector("canvas");
    // eframe adds its hidden text <input> once the app runs; then it listens for drops.
    var running = canvas && document.querySelector("input[type=text]");
    if (!running && tries < 150) {
      setTimeout(function () { deliver(tries + 1); }, 100);
      return;
    }
    if (!canvas) return;
    var json = bridge.takeFile && bridge.takeFile();
    if (!json) return;
    var f;
    try { f = JSON.parse(json); } catch (err) { return; }
    if (readOnly) readOnlySteps(true);
    var file = new File([b64ToBytes(f.data)], f.name, { type: f.type || "" });
    var dt = new DataTransfer();
    dt.items.add(file);
    var r = canvas.getBoundingClientRect();
    var at = { bubbles: true, cancelable: true, dataTransfer: dt, clientX: r.left + r.width / 2, clientY: r.top + r.height / 2 };
    canvas.dispatchEvent(new DragEvent("dragover", at));
    canvas.dispatchEvent(new DragEvent("drop", at));
  }

  if (bridge.hasFile && bridge.hasFile()) {
    // After the app's first frames (its fonts and settings), so the file opens in a ready app.
    var go = function () { setTimeout(function () { deliver(0); }, 400); };
    if (document.readyState === "complete") go(); else window.addEventListener("load", go);
  }
})();
