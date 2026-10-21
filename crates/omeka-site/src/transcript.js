/*
 * The transcript's script. Every cue is already a link that reloads the page
 * with `?t=`, so this adds only what a static page cannot do: seek in place,
 * keep the playing line in view, and draw an `<audio>` element's captions,
 * which a browser never draws itself.
 *
 * The "Follow along" checkbox ships hidden and checked, so a reader with no
 * script never sees a control that does nothing.
 */
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

  function wire(section) {
    if (section.hasAttribute('data-transcript-wired')) {
      return;
    }
    section.setAttribute('data-transcript-wired', '');

    var player = (section.parentNode || document).querySelector('video, audio');
    var box = section.querySelector('.transcript-lines');
    var toggle = section.querySelector('[data-transcript-follow]');
    var band = (section.parentNode || document).querySelector('[data-transcript-band]');
    if (!player || !box) {
      return;
    }

    var lines = [];
    var nodes = box.querySelectorAll('.transcript-line');
    for (var index = 0; index < nodes.length; index += 1) {
      lines.push({
        node: nodes[index],
        start: Number(nodes[index].getAttribute('data-start-ms')) / 1000,
        end: Number(nodes[index].getAttribute('data-end-ms')) / 1000
      });
    }
    if (lines.length === 0) {
      return;
    }

    var active = -1;

    function following() {
      var input = toggle && toggle.querySelector('input');
      return Boolean(input && input.checked);
    }

    function keepInView(node) {
      var boxRect = box.getBoundingClientRect();
      var lineRect = node.getBoundingClientRect();
      box.scrollTop = Math.max(
        0,
        box.scrollTop + lineRect.top - boxRect.top - (box.clientHeight - lineRect.height) / 2
      );
    }

    function mark(index) {
      if (index === active) {
        return;
      }
      if (active >= 0) {
        var previous = lines[active].node;
        previous.classList.remove('is-playing');
        previous.querySelector('.transcript-cue').removeAttribute('aria-current');
      }
      active = index;
      if (index < 0) {
        if (band) {
          band.textContent = '';
          band.hidden = true;
        }
        return;
      }
      var node = lines[index].node;
      node.classList.add('is-playing');
      node.querySelector('.transcript-cue').setAttribute('aria-current', 'true');
      if (band) {
        var text = node.querySelector('.transcript-text');
        band.textContent = text ? text.textContent : '';
        band.hidden = false;
      }
      if (following()) {
        keepInView(node);
      }
    }

    function markAt(seconds) {
      for (var index = 0; index < lines.length; index += 1) {
        if (seconds >= lines[index].start && seconds < lines[index].end) {
          mark(index);
          return;
        }
      }
      mark(-1);
    }

    function lineOf(link) {
      for (var index = 0; index < lines.length; index += 1) {
        if (lines[index].node === link.parentNode) {
          return index;
        }
      }
      return -1;
    }

    function stopFollowing() {
      var input = toggle && toggle.querySelector('input');
      if (input && input.checked) {
        input.checked = false;
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
      var start = Number(link.parentNode.getAttribute('data-start-ms')) / 1000;
      if (Number.isNaN(start)) {
        return;
      }
      try {
        player.currentTime = start;
      } catch (error) {
        return;
      }
      mark(lineOf(link));
      var url = new URL(window.location.href);
      url.searchParams.set('t', String(start));
      window.history.replaceState(null, '', url);
      var playing = player.play();
      if (playing && typeof playing.catch === 'function') {
        playing.catch(function () {});
      }
    });

    box.addEventListener(
      'wheel',
      function () {
        stopFollowing();
      },
      { passive: true }
    );
    box.addEventListener(
      'touchmove',
      function () {
        stopFollowing();
      },
      { passive: true }
    );
    box.addEventListener('keydown', function (event) {
      if (SCROLL_KEYS[event.key]) {
        stopFollowing();
      }
    });

    player.addEventListener('timeupdate', function () {
      markAt(player.currentTime);
    });
    player.addEventListener('seeked', function () {
      markAt(player.currentTime);
    });
  }

  var sections = document.querySelectorAll('[data-transcript]');
  for (var index = 0; index < sections.length; index += 1) {
    wire(sections[index]);
  }
})();
