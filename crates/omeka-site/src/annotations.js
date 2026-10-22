// Links an annotation list to the picture or the recording it names: hover and
// focus mark the paired region, a time note seeks the player, and a cited note
// that is gone says so. The same file wires a recording's transcript, so the
// page has one way to seek the player and one way to mark the playing line.
// Without this file the page still works: the list's place links jump to the
// region, and a time note or a cue link reloads at `?t=`.
(function () {
  'use strict';

  var SCROLL_KEYS = {
    ArrowUp: true,
    ArrowDown: true,
    PageUp: true,
    PageDown: true,
    Home: true,
    End: true,
    ' ': true
  };

  function mediaIn(root) {
    return root.querySelector('video, audio');
  }

  // The one seek: a link that names a moment in its enclosing entry moves the
  // player there and leaves the address at that moment, so a copied URL opens
  // the same place. The link's own `?t=` is the address it leaves.
  function seek(media, entry, link) {
    if (!media || !entry) {
      return false;
    }
    var start = Number(entry.getAttribute('data-start-ms'));
    if (!isFinite(start)) {
      return false;
    }
    try {
      media.currentTime = start / 1000;
    } catch (error) {
      return false;
    }
    var played = media.play();
    if (played && played.catch) {
      played.catch(function () {});
    }
    var href = link.getAttribute('href') || '';
    var fragment = href.indexOf('#');
    if (fragment >= 0) {
      history.replaceState(null, '', href.slice(0, fragment));
    }
    return true;
  }

  // The one mark of the playing time. The entry whose range holds the moment is
  // current, and `aria-current` and `.is-playing` sit on the entry itself, so a
  // note and a transcript line read the same way to a screen reader.
  function markEntry(entry, on) {
    entry.classList.toggle('is-playing', on);
    if (on) {
      entry.setAttribute('aria-current', 'true');
    } else {
      entry.removeAttribute('aria-current');
    }
  }

  function playingLine(entries, milliseconds) {
    var current = null;
    entries.forEach(function (entry) {
      var start = Number(entry.getAttribute('data-start-ms'));
      var end = Number(entry.getAttribute('data-end-ms'));
      var inside = milliseconds >= start && milliseconds < end;
      markEntry(entry, inside);
      if (inside) {
        current = entry;
      }
    });
    return current;
  }

  function markOnly(entries, target) {
    entries.forEach(function (entry) {
      markEntry(entry, entry === target);
    });
  }

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

  function playing(figure) {
    var media = mediaIn(figure);
    if (!media) {
      return;
    }
    var entries = Array.prototype.slice.call(
      figure.querySelectorAll('.annotation-item[data-start-ms][data-end-ms]')
    );
    // A rail mark, and on a video a box and its marker, play when their entry
    // does: the stylesheet shows a video's box only then.
    var followers = figure.querySelectorAll(
      '.note-rail-mark, .annotated-video .annotation-region, .annotated-video .note-marker'
    );
    var update = function () {
      playingLine(entries, media.currentTime * 1000);
      followers.forEach(function (follower) {
        var entry = entryOf(figure, follower.getAttribute('data-note'));
        follower.classList.toggle(
          'is-playing',
          Boolean(entry && entry.classList.contains('is-playing'))
        );
      });
    };
    media.addEventListener('timeupdate', update);
    media.addEventListener('seeked', update);
    update();
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
        if (seek(mediaIn(figure), link.closest('[data-start-ms]'), link)) {
          event.preventDefault();
        }
      });
    });
    if (
      figure.getAttribute('data-duration-ms') ||
      figure.classList.contains('annotated-video')
    ) {
      playing(figure);
    }
    cited(figure);
  }

  function keepInView(box, line) {
    var boxRect = box.getBoundingClientRect();
    var lineRect = line.getBoundingClientRect();
    box.scrollTop = Math.max(
      0,
      box.scrollTop + lineRect.top - boxRect.top - (box.clientHeight - lineRect.height) / 2
    );
  }

  function followToggle(section) {
    return section.querySelector('[data-transcript-follow]');
  }

  function following(section) {
    var toggle = followToggle(section);
    var input = toggle && toggle.querySelector('input');
    return Boolean(input && input.checked);
  }

  function stopFollowing(section) {
    var toggle = followToggle(section);
    var input = toggle && toggle.querySelector('input');
    if (input && input.checked) {
      input.checked = false;
    }
  }

  // A recording's transcript, beside the player it reads. The seek and the mark
  // are the note list's; this adds the band an `<audio>` element has no area to
  // draw, and keeping the playing line in its own scrolling box.
  function wireTranscript(section) {
    if (section.getAttribute('data-transcript-wired')) {
      return;
    }
    section.setAttribute('data-transcript-wired', '');
    var scope = section.parentNode || document;
    var media = mediaIn(scope);
    var box = section.querySelector('.transcript-lines');
    var toggle = followToggle(section);
    var band = scope.querySelector('[data-transcript-band]');
    if (!media || !box) {
      return;
    }
    var entries = Array.prototype.slice.call(box.querySelectorAll('.transcript-line'));
    if (entries.length === 0) {
      return;
    }
    var active = null;

    function show(line) {
      if (line === active) {
        return;
      }
      active = line;
      if (band) {
        var text = line && line.querySelector('.transcript-text');
        band.textContent = text ? text.textContent : '';
        band.hidden = !line;
      }
      if (line && following(section)) {
        keepInView(box, line);
      }
    }

    if (toggle) {
      toggle.hidden = false;
    }

    box.addEventListener('click', function (event) {
      var link = event.target.closest ? event.target.closest('a.transcript-cue') : null;
      if (!link || !box.contains(link)) {
        return;
      }
      if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) {
        return;
      }
      event.preventDefault();
      var line = link.parentNode;
      if (seek(media, line, link)) {
        markOnly(entries, line);
        show(line);
      }
    });

    ['wheel', 'touchmove'].forEach(function (type) {
      box.addEventListener(
        type,
        function () {
          stopFollowing(section);
        },
        { passive: true }
      );
    });
    box.addEventListener('keydown', function (event) {
      if (SCROLL_KEYS[event.key]) {
        stopFollowing(section);
      }
    });

    media.addEventListener('timeupdate', function () {
      show(playingLine(entries, media.currentTime * 1000));
    });
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
    document.querySelectorAll('[data-transcript]').forEach(wireTranscript);
    activeFromHash();
  }

  window.addEventListener('hashchange', activeFromHash);
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
