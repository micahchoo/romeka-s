/**
 * The Spanish jump-to box and saved list messages, written by hand.
 *
 * It is typed `FindingMessages`, so a key added to English and not here is a
 * `tsc` error.
 */
import type { FindingMessages } from '../index';

export const es: FindingMessages = {
  'jump.loading': 'Buscando «{text}»…',
  'jump.none': 'Ningún registro que usted pueda ver coincide con «{text}».',
  'jump.many': '{count} registros coinciden con «{text}»',
  'jump.firstTen': 'Se muestran los {count} primeros. Escriba más del título para acotarlos.',
  'jump.failed': 'La búsqueda no terminó: {message}',
  'jump.close': 'Cerrar',
  'jump.kind.items': 'Elemento',
  'jump.kind.item_sets': 'Conjunto de elementos',
  'jump.kind.media': 'Medio',
  'jump.untitled': '[Sin título]',
  'savedLists.save': 'Guardar esta lista',
  'savedLists.name': 'Nombre',
  'savedLists.where':
    'Guarda estos filtros y este orden. El nombre aparece bajo Elementos en el menú, en cualquier equipo en el que inicie sesión.',
  'savedLists.submit': 'Guardar',
  'savedLists.saving': 'Guardando…',
  'savedLists.cancel': 'Cancelar',
  'savedLists.current': 'Lista guardada: {name}',
  'savedLists.rename': 'Cambiar el nombre de la lista guardada',
  'savedLists.delete': 'Eliminar la lista guardada',
  'savedLists.deleted': 'Se eliminó la lista guardada {name}.',
  'savedLists.nameEmpty': 'Dé un nombre a la lista.',
  'savedLists.nameTaken': 'Ya tiene una lista llamada {name}.',
  'savedLists.tooMany': 'Tiene {count} listas guardadas. Es el máximo que una persona puede tener. Elimine una primero.',
  'savedLists.gone': 'Esta lista guardada se eliminó en otra ventana.',
};
