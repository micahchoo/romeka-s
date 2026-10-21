// Links an annotation list to the picture or the recording it names: hover and
// focus mark the paired region, a time note seeks the player, and a cited note
// that is gone says so. Without this file the page still works: the list's
// place links jump to the region and a time link reloads at `?t=`.
(function () {
  'use strict';

  function regionsOf(figure) {
    return figure.querySelectorAll('[data-note]');
  }

  function entryOf(figure, id) {
    return figure.querySelector('.annotation-item[data-note="' + id + '"]');
  }

  function mark(figure, id, on) {
    regionsOf(figure).forEach(function (element) {
      if (element.getAttribute('data-note') === id) {
        element.classList.toggle('is-active', on);
      }
    });
  }

  function seek(figure, link) {
    var media = figure.querySelector('video, audio');
    var entry = link.closest('[data-start-ms]');
    if (!media || !entry) {
      return false;
    }
    var start = Number(entry.getAttribute('data-start-ms'));
    if (!isFinite(start)) {
      return false;
    }
    media.currentTime = start / 1000;
    var played = media.play();
    if (played && played.catch) {
      played.catch(function () {});
    }
    var href = link.getAttribute('href') || '';
    var at = href.indexOf('#');
    history.replaceState(null, '', at < 0 ? href : href.slice(0, at));
    return true;
  }

  function playing(figure) {
    var media = figure.querySelector('video, audio');
    if (!media) {
      return;
    }
    var entries = figure.querySelectorAll('[data-start-ms][data-end-ms]');
    media.addEventListener('timeupdate', function () {
      var now = media.currentTime * 1000;
      entries.forEach(function (entry) {
        var start = Number(entry.getAttribute('data-start-ms'));
        var end = Number(entry.getAttribute('data-end-ms'));
        var inside = now >= start && now < end;
        entry.classList.toggle('is-playing', inside);
        if (inside) {
          entry.setAttribute('aria-current', 'true');
        } else {
          entry.removeAttribute('aria-current');
        }
        var id = entry.getAttribute('data-note');
        figure.querySelectorAll('.note-rail-mark').forEach(function (mark) {
          if (mark.getAttribute('data-note') === id) {
            mark.classList.toggle('is-playing', inside);
          }
        });
      });
    });
  }

  function cited(figure) {
    var id = figure.getAttribute('data-cited');
    if (!id) {
      return;
    }
    if (!document.getElementById('note-' + id)) {
      var missing = figure.querySelector('.annotation-missing');
      if (missing) {
        missing.hidden = false;
      }
    }
  }

  function wire(figure) {
    if (figure.getAttribute('data-notes-wired')) {
      return;
    }
    figure.setAttribute('data-notes-wired', '');
    var entries = figure.querySelectorAll('.annotation-item');
    entries.forEach(function (entry) {
      var id = entry.getAttribute('data-note');
      var show = function () {
        mark(figure, id, true);
      };
      var hide = function () {
        mark(figure, id, false);
      };
      entry.addEventListener('mouseenter', show);
      entry.addEventListener('mouseleave', hide);
      entry.addEventListener('focusin', show);
      entry.addEventListener('focusout', hide);
    });
    regionsOf(figure).forEach(function (region) {
      var id = region.getAttribute('data-note');
      var entry = entryOf(figure, id);
      if (!entry) {
        return;
      }
      region.addEventListener('mouseenter', function () {
        entry.classList.add('is-active');
      });
      region.addEventListener('mouseleave', function () {
        entry.classList.remove('is-active');
      });
    });
    figure.querySelectorAll('.note-time').forEach(function (link) {
      link.addEventListener('click', function (event) {
        if (seek(figure, link)) {
          event.preventDefault();
        }
      });
    });
    if (figure.getAttribute('data-duration-ms')) {
      playing(figure);
    }
    cited(figure);
  }

  function activeFromHash() {
    var match = /^#note-(\d+)$/.exec(window.location.hash || '');
    if (!match) {
      return;
    }
    document.querySelectorAll('.annotated-media').forEach(function (figure) {
      var entry = entryOf(figure, match[1]);
      if (entry) {
        mark(figure, match[1], true);
      }
    });
  }

  function init() {
    document.querySelectorAll('.annotated-media').forEach(wire);
    activeFromHash();
  }

  window.addEventListener('hashchange', activeFromHash);
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
