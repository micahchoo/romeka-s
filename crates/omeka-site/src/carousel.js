// Steps each browse preview carousel one screen at a time and marks the end
// it has reached. Without this script the previous and next links still
// work: they jump to the first card and to the first card past one screen.
(function () {
  'use strict';
  var reduce = window.matchMedia('(prefers-reduced-motion: reduce)');
  document.querySelectorAll('.preview-carousel').forEach(function (region) {
    var track = region.querySelector('.resource-carousel');
    var prev = region.querySelector('.preview-carousel-prev');
    var next = region.querySelector('.preview-carousel-next');
    if (!track || !prev || !next) {
      return;
    }
    // A right-to-left track scrolls toward negative offsets.
    var sign = getComputedStyle(track).direction === 'rtl' ? -1 : 1;
    var update = function () {
      var offset = Math.abs(track.scrollLeft);
      var end = track.scrollWidth - track.clientWidth - 1;
      prev.setAttribute('aria-disabled', String(offset <= 0));
      next.setAttribute('aria-disabled', String(offset >= end));
    };
    var step = function (direction) {
      track.scrollBy({
        left: sign * direction * track.clientWidth,
        behavior: reduce.matches ? 'auto' : 'smooth',
      });
    };
    prev.addEventListener('click', function (event) {
      event.preventDefault();
      step(-1);
    });
    next.addEventListener('click', function (event) {
      event.preventDefault();
      step(1);
    });
    track.addEventListener('scroll', update, { passive: true });
    update();
  });
})();
