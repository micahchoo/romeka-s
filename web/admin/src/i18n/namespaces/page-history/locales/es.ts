/**
 * The Spanish page history messages, written by hand.
 *
 * It is typed `PageHistoryMessages`, so a key added to English and not here is a
 * `tsc` error.
 */
import type { PageHistoryMessages } from '../index';

export const es: PageHistoryMessages = {
  'history.title': 'Historial de la página',
  'history.close': 'Cerrar',
  'history.loading': 'Cargando las versiones...',
  'history.empty': 'Aún no hay versiones. Cada guardado conserva una.',
  'history.entry': '{time} · {name} · {blocks}',
  'history.today': 'Hoy {time}',
  'history.notRecorded': 'Autor no registrado',
  'history.deletedUser': 'Usuario eliminado',
  'history.firstVersion': 'Primera versión guardada',
  'history.created': 'Creada',
  'history.olderGone': 'Las versiones anteriores ya no se conservan',
  'history.noChanges': 'Sin cambios',
  'history.retitled': 'Título cambiado de «{before}» a «{after}»',
  'history.settingsChanged': 'Ajustes de la página cambiados',
  'history.addedOne': '1 bloque añadido',
  'history.added': '{count} bloques añadidos',
  'history.removedOne': '1 bloque eliminado',
  'history.removed': '{count} bloques eliminados',
  'history.changedOne': '1 bloque cambiado',
  'history.changed': '{count} bloques cambiados',
  'history.changedNamedOne': '1 bloque cambiado: {names}',
  'history.changedNamed': '{count} bloques cambiados: {names}',
  'history.movedOne': '1 bloque movido',
  'history.moved': '{count} bloques movidos',
  'history.live': 'Publicada ahora',
  'history.viewing': 'Viendo la versión del {time}',
  'history.restore': 'Restaurar esta versión',
  'history.back': 'Volver al borrador actual',
  'history.rendering': 'Mostrando esta versión...',
  'history.renderFailed': 'No se pudo mostrar esta versión.',
  'history.conflictTitle': 'Otra persona guardó esta página',
  'history.conflict': 'Guardada por {name} a las {time} después de que abriera esta página',
  'history.conflictUntimed': 'Guardada por {name} después de que abriera esta página',
  'history.conflictChoices':
    'La versión de esa persona se abre en el editor y la suya queda a un Deshacer de distancia. «Conservar la mía» guarda la suya encima de la otra.',
  'history.someoneElse': 'otra persona',
  'history.showTheirs': 'Ver su versión',
  'history.keepMine': 'Conservar la mía',
};
