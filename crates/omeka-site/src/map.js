// Draws every map a page holds with the vendored Leaflet. The server writes
// each map's settings and places as JSON in `data-romeka-map`; the page keeps
// a list of the places as links, so a reader without scripts loses nothing.
(function () {
  'use strict';
  var L = window.L;
  if (!L) {
    return;
  }

  // Close markers join when they fall in the same square of this many pixels.
  var CELL = 56;

  function safeHref(url) {
    return typeof url === 'string' && /^(\/(?!\/)|https?:\/\/)/.test(url) ? url : null;
  }

  function text(tag, className, value) {
    var node = document.createElement(tag);
    node.className = className;
    node.textContent = value;
    return node;
  }

  function popup(point, options) {
    var box = document.createElement('div');
    box.className = 'romeka-map-popup';
    var href = safeHref(point.url);
    if (point.thumbnail && safeHref(point.thumbnail)) {
      var image = document.createElement('img');
      image.src = point.thumbnail;
      image.alt = '';
      box.appendChild(image);
    }
    if (options.title) {
      var title = href ? document.createElement('a') : document.createElement('strong');
      title.className = 'romeka-map-popup-title';
      title.textContent = point.title;
      if (href) {
        title.href = href;
      }
      box.appendChild(title);
    }
    if (point.value) {
      box.appendChild(text('p', 'romeka-map-popup-value', point.value));
    }
    return box;
  }

  function pin(className, html, size) {
    return L.divIcon({ className: className, html: html, iconSize: [size, size] });
  }

  // Grid clustering at the current zoom: points whose projected pixels share a
  // cell become one marker at their mean position.
  function groups(map, points) {
    var zoom = map.getZoom();
    var cells = {};
    var order = [];
    points.forEach(function (point) {
      var pixel = map.project([point.lat, point.lng], zoom);
      var key = Math.floor(pixel.x / CELL) + ':' + Math.floor(pixel.y / CELL);
      if (!cells[key]) {
        cells[key] = [];
        order.push(key);
      }
      cells[key].push(point);
    });
    return order.map(function (key) {
      return cells[key];
    });
  }

  function marker(point, options) {
    return L.marker([point.lat, point.lng], {
      icon: pin('romeka-map-pin', '', 18),
      title: point.title,
      alt: point.title,
      keyboard: true,
    }).bindPopup(popup(point, options));
  }

  function clusterMarker(map, group, options) {
    var lat = 0;
    var lng = 0;
    group.forEach(function (point) {
      lat += point.lat;
      lng += point.lng;
    });
    var label = options.clusterLabel.replace('%s', String(group.length));
    var count = document.createElement('span');
    count.textContent = String(group.length);
    var cluster = L.marker([lat / group.length, lng / group.length], {
      icon: pin('romeka-map-cluster', count.outerHTML, 36),
      title: label,
      alt: label,
      keyboard: true,
    });
    var bounds = L.latLngBounds(group.map(function (point) {
      return [point.lat, point.lng];
    }));
    cluster.on('click', function () {
      if (map.getZoom() >= map.getMaxZoom() || bounds.getNorthEast().equals(bounds.getSouthWest())) {
        var list = document.createElement('div');
        list.className = 'romeka-map-popup-list';
        group.forEach(function (point) {
          list.appendChild(popup(point, options));
        });
        cluster.bindPopup(list).openPopup();
        return;
      }
      map.fitBounds(bounds.pad(0.2));
    });
    return cluster;
  }

  function draw(element) {
    var settings;
    try {
      settings = JSON.parse(element.getAttribute('data-romeka-map') || '');
    } catch (error) {
      return;
    }
    var points = Array.isArray(settings.points) ? settings.points : [];
    var map = L.map(element, { scrollWheelZoom: false });
    L.tileLayer(settings.tiles, { maxZoom: 19 }).addTo(map);
    map.attributionControl.addAttribution(text('span', '', settings.attribution).outerHTML);
    var options = { title: settings.showTitle !== false, clusterLabel: settings.clusterLabel || '%s' };
    var layer = L.layerGroup().addTo(map);
    function redraw() {
      layer.clearLayers();
      if (!settings.cluster) {
        points.forEach(function (point) {
          layer.addLayer(marker(point, options));
        });
        return;
      }
      groups(map, points).forEach(function (group) {
        layer.addLayer(group.length === 1 ? marker(group[0], options) : clusterMarker(map, group, options));
      });
    }
    var view = settings.view || {};
    if (typeof view.lat === 'number' && typeof view.lng === 'number') {
      map.setView([view.lat, view.lng], typeof view.zoom === 'number' ? view.zoom : 5);
    } else if (points.length === 1) {
      map.setView([points[0].lat, points[0].lng], 12);
    } else if (points.length > 1) {
      map.fitBounds(points.map(function (point) {
        return [point.lat, point.lng];
      }), { padding: [24, 24], maxZoom: 14 });
    } else {
      map.setView([20, 0], 1);
    }
    redraw();
    if (settings.cluster) {
      map.on('zoomend', redraw);
    }
    element.classList.add('romeka-map-ready');
  }

  function start() {
    Array.prototype.forEach.call(document.querySelectorAll('[data-romeka-map]'), draw);
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', start);
  } else {
    start();
  }
})();
