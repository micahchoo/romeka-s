/**
 * The Spanish item history messages, written by hand.
 *
 * It is typed `ItemHistoryMessages`, so a key added to English and not here is
 * a `tsc` error.
 */
import type { ItemHistoryMessages } from '../index';

export const es: ItemHistoryMessages = {
  'itemHistory.loading': 'Leyendo el historial…',
  'itemHistory.empty': 'Todavía no hay ningún cambio registrado en este elemento.',
  'itemHistory.entry': '{date} · {who}',
  'itemHistory.entrySource': '{date} · {who} · {source}',
  'itemHistory.deletedUser': 'Un usuario eliminado',
  'itemHistory.source.bulk': 'Cambio masivo {id}',
  'itemHistory.source.import': 'Importación {id}',
  'itemHistory.source.bulkUndo': 'Deshacer el cambio masivo {id}',
  'itemHistory.source.importUndo': 'Deshacer la importación {id}',
  'itemHistory.change': '{field}: {before} → {after}',
  'itemHistory.field.visibility': 'Visibilidad',
  'itemHistory.field.class': 'Clase',
  'itemHistory.field.template': 'Tipo de elemento',
  'itemHistory.field.itemSets': 'Conjuntos de elementos',
  'itemHistory.field.sites': 'Sitios',
  'itemHistory.public': 'Público',
  'itemHistory.private': 'Privado',
  'itemHistory.none': '(ninguno)',
  'itemHistory.inTrash': '{name} (en la papelera)',
  'itemHistory.deletedSet': 'Conjunto de elementos eliminado {id}',
  'itemHistory.privateValue': '{value} (privado)',
  'itemHistory.putBack': 'Restaurar este valor',
  'itemHistory.puttingBack': 'Restaurando el valor…',
  'itemHistory.putBackDone': 'El valor ha vuelto. El cambio más reciente, arriba, lo indica.',
  'itemHistory.putBackSame': 'No cambió nada: el elemento ya tiene este valor.',
  'itemHistory.older': 'Mostrar cambios anteriores',
};
