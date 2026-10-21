//! The 3D reader's facade script, served as `js/model.js`.
//!
//! The model file renderer emits the poster, the button and the deferred
//! `<model-viewer>` element; this script is what the button runs. It is a theme
//! file rather than inline code, so the page works under a `script-src 'self'`
//! policy, and the vendored library is a script this file adds only when the
//! reader presses the button.

/// The facade: one listener per model reader on the page, no shared state.
pub const SCRIPT: &str = r#"(function () {
  var WIRED = 'data-model-wired';
  function fail(root) {
    var button = root.querySelector('.model-turn');
    var failure = root.querySelector('.model-error');
    if (button) button.hidden = true;
    if (failure) failure.hidden = false;
    root.setAttribute('data-model-failed', '');
  }
  function wire(root) {
    if (root.getAttribute(WIRED)) return;
    root.setAttribute(WIRED, '');
    var button = root.querySelector('.model-turn');
    var viewer = root.querySelector('model-viewer');
    var poster = root.querySelector('.model-poster');
    if (!button || !viewer) return;
    if (!window.customElements || !window.customElements.whenDefined) { fail(root); return; }
    button.addEventListener('click', function () {
      button.disabled = true;
      var loading = root.getAttribute('data-loading');
      if (loading) button.textContent = loading;
      var library = document.createElement('script');
      library.src = root.getAttribute('data-library');
      library.addEventListener('error', function () { fail(root); });
      document.head.appendChild(library);
      window.customElements.whenDefined('model-viewer').then(function () {
        viewer.addEventListener('load', function () {
          root.setAttribute('data-model-loaded', '');
          if (poster) poster.hidden = true;
          button.hidden = true;
        });
        viewer.addEventListener('error', function () { fail(root); });
        viewer.hidden = false;
        viewer.src = viewer.getAttribute('data-src');
      }, function () { fail(root); });
    });
  }
  function init() {
    var roots = document.querySelectorAll('[data-model-reader]');
    for (var index = 0; index < roots.length; index += 1) wire(roots[index]);
  }
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
"#;

#[cfg(test)]
mod tests {
    use super::*;

    /// The script loads the vendored library from the URL the page names, and
    /// never builds one itself: a policy that allows only this origin's files
    /// runs it unchanged.
    #[test]
    fn the_facade_names_the_library_and_creates_its_script() {
        assert!(SCRIPT.contains("document.createElement('script')"));
        assert!(SCRIPT.contains("root.getAttribute('data-library')"));
        assert!(SCRIPT.contains("customElements.whenDefined('model-viewer')"));
        assert!(!SCRIPT.contains("eval("), "the facade needs no eval");
    }
}
