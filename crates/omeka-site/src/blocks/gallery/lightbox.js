// Opens a gallery's images in a lightbox. Each image is a link to its large
// file, so a reader without scripts opens the file itself; with this script
// the same link opens a modal dialog that steps through the gallery. The
// server marks a gallery that wants the lightbox with `data-romeka-lightbox`
// and writes the dialog's words beside it.
(function () {
  'use strict';
  if (typeof HTMLDialogElement !== 'function') {
    return;
  }

  function button(className, label, glyph) {
    var node = document.createElement('button');
    node.type = 'button';
    node.className = 'romeka-lightbox-button ' + className;
    node.setAttribute('aria-label', label);
    node.textContent = glyph;
    return node;
  }

  function position(pattern, index, count) {
    return pattern.replace('{index}', String(index + 1)).replace('{count}', String(count));
  }

  function open(gallery, links, start) {
    var labels = gallery.dataset;
    var dialog = document.createElement('dialog');
    dialog.className = 'romeka-lightbox';
    var figure = document.createElement('figure');
    figure.className = 'romeka-lightbox-figure';
    var image = document.createElement('img');
    image.className = 'romeka-lightbox-image';
    var caption = document.createElement('figcaption');
    caption.className = 'romeka-lightbox-caption';
    var status = document.createElement('p');
    status.className = 'romeka-lightbox-position';
    status.setAttribute('aria-live', 'polite');
    figure.appendChild(image);
    figure.appendChild(caption);
    var close = button('romeka-lightbox-close', labels.close || 'Close', '×');
    var previous = button('romeka-lightbox-previous', labels.previous || 'Previous', '‹');
    var next = button('romeka-lightbox-next', labels.next || 'Next', '›');
    dialog.appendChild(close);
    dialog.appendChild(figure);
    dialog.appendChild(status);
    if (links.length > 1) {
      dialog.appendChild(previous);
      dialog.appendChild(next);
    }
    var index = start;

    function show(at) {
      index = (at + links.length) % links.length;
      var link = links[index];
      var source = link.querySelector('img');
      image.src = link.href;
      image.alt = source ? source.alt : '';
      var text = link.closest('figure');
      var shown = text ? text.querySelector('figcaption') : null;
      caption.innerHTML = shown ? shown.innerHTML : '';
      caption.hidden = !shown;
      var where = position(labels.position || '{index} / {count}', index, links.length);
      status.textContent = where;
      dialog.setAttribute('aria-label', where);
    }

    close.addEventListener('click', function () {
      dialog.close();
    });
    previous.addEventListener('click', function () {
      show(index - 1);
    });
    next.addEventListener('click', function () {
      show(index + 1);
    });
    dialog.addEventListener('keydown', function (event) {
      if (event.key === 'ArrowLeft') {
        show(index - 1);
      } else if (event.key === 'ArrowRight') {
        show(index + 1);
      }
    });
    dialog.addEventListener('click', function (event) {
      if (event.target === dialog) {
        dialog.close();
      }
    });
    dialog.addEventListener('close', function () {
      dialog.remove();
      links[index].focus();
    });
    show(start);
    document.body.appendChild(dialog);
    dialog.showModal();
    close.focus();
  }

  document.querySelectorAll('[data-romeka-lightbox]').forEach(function (gallery) {
    var links = Array.prototype.slice.call(gallery.querySelectorAll('a.romeka-gallery-link'));
    links.forEach(function (link, at) {
      link.addEventListener('click', function (event) {
        if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey) {
          return;
        }
        event.preventDefault();
        open(gallery, links, at);
      });
    });
  });
})();
