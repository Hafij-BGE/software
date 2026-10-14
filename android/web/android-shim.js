// Craft Suite for Android: hands the web apps' downloads (how they save a file) to the app, which
// asks where to save it. A WebView doesn't download blob: links itself. Loaded before each app's
// own scripts (packaging/build-apk.sh adds it to their index.html).
(function () {
  "use strict";
  if (window.__craftSuiteShim || !window.CraftSuiteAndroid) return;
  window.__craftSuiteShim = true;

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
    var reader = new FileReader();
    reader.onload = function () {
      var data = String(reader.result);
      window.CraftSuiteAndroid.save(name, blob.type || "", data.substring(data.indexOf(",") + 1));
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
})();
