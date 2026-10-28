/**
 * The Spanish bulk delete messages, written by hand.
 *
 * It is typed `BulkDeleteMessages`, so a key added to English and not here is
 * a `tsc` error.
 */
import type { BulkDeleteMessages } from '../index';

export const es: BulkDeleteMessages = {
  'bulkDelete.scope.refused':
    'Su rol solo puede eliminar elementos marcados. Marque los elementos que quiere eliminar.',
  'bulkDelete.ask.note': 'La vista previa muestra lo que se va. Nada pasa a la papelera hasta que lo confirme.',
  'bulkDelete.confirm.itemsOne': '1 elemento pasa a la papelera.',
  'bulkDelete.confirm.itemsMany': '{count} elementos pasan a la papelera.',
  'bulkDelete.confirm.none': 'Ninguno de estos elementos es uno que usted pueda eliminar.',
  'bulkDelete.confirm.mediaOne': 'También se va 1 archivo multimedia: {size}.',
  'bulkDelete.confirm.mediaMany': 'También se van {count} archivos multimedia: {size} en total.',
  'bulkDelete.confirm.deniedOne': '1 elemento que usted no puede eliminar se queda como está.',
  'bulkDelete.confirm.deniedMany': '{count} elementos que usted no puede eliminar se quedan como están.',
  'bulkDelete.confirm.samples': 'Los primeros elementos que se van',
  'bulkDelete.confirm.trash':
    'Se quedan en la papelera durante {days} días. Hasta entonces, Deshacer los devuelve con sus archivos multimedia.',
  'bulkDelete.confirm.type': 'Para confirmar, escriba el número de elementos, {count}',
  'bulkDelete.confirm.moveOne': 'Mover 1 elemento a la papelera',
  'bulkDelete.confirm.moveMany': 'Mover {count} elementos a la papelera',
  'bulkDelete.empty.open': 'Vaciar la papelera…',
  'bulkDelete.empty.confirm': 'Vaciar la papelera',
  'bulkDelete.empty.askAll':
    'Todo lo que hay en la papelera se elimina para siempre: los elementos con sus medios y archivos, los medios con su archivo y los conjuntos de elementos. Esas eliminaciones ya no se podrán deshacer.',
  'bulkDelete.empty.cancel': 'Cancelar',
  'bulkDelete.empty.emptied': 'La papelera está vacía.',
  'bulkDelete.empty.itemsOne': 'Se eliminó para siempre 1 elemento.',
  'bulkDelete.empty.itemsMany': 'Se eliminaron para siempre {count} elementos.',
  'bulkDelete.empty.mediaOne': 'Se eliminó para siempre 1 medio.',
  'bulkDelete.empty.mediaMany': 'Se eliminaron para siempre {count} medios.',
  'bulkDelete.empty.setsOne': 'Se eliminó para siempre 1 conjunto de elementos.',
  'bulkDelete.empty.setsMany': 'Se eliminaron para siempre {count} conjuntos de elementos.',
  'bulkDelete.empty.nothing': 'La papelera ya estaba vacía.',
  'bulkDelete.item.checking': 'Comprobando lo que se va con este elemento…',
  'bulkDelete.item.cannotCheck':
    'No se puede comprobar lo que se va con este elemento, así que ahora no puede pasar a la papelera.',
  'bulkDelete.item.checkAgain': 'Comprobar de nuevo',
  'bulkDelete.item.denied': 'Usted no puede eliminar este elemento.',
  'bulkDelete.item.ask': '¿Mover {name} a la papelera?',
  'bulkDelete.item.linked': 'Otros registros enlazan con él:',
  'bulkDelete.item.linksBack': 'Sus enlaces vuelven si usted lo restaura.',
  'bulkDelete.item.notLinked': 'Nada enlaza con él.',
  'bulkDelete.item.trash':
    'La papelera lo guarda durante {days} días. Hasta entonces, Deshacer en la pantalla Elementos lo devuelve con sus archivos multimedia.',
  'bulkDelete.item.move': 'Mover a la papelera',
  'bulkDelete.item.moving': 'Moviendo a la papelera…',
  'bulkDelete.item.failed': '{name} no pasó a la papelera. Se queda como está.',
  'bulkDelete.item.trashed': 'Se eliminó {name}.',
  'bulkDelete.item.undo': 'Deshacer',
  'bulkDelete.item.undoing': 'Devolviendo {name}…',
  'bulkDelete.item.back': '{name} ha vuelto.',
  'bulkDelete.item.notBack': '{name} se queda en la papelera.',
  'bulkDelete.media.checking': 'Comprobando lo que se va con este medio…',
  'bulkDelete.media.cannotCheck':
    'No se puede comprobar lo que se va con este medio, así que ahora no puede pasar a la papelera.',
  'bulkDelete.media.denied': 'No puede eliminar este medio.',
  'bulkDelete.media.file': 'Su archivo se va con él: {size}.',
  'bulkDelete.media.trash':
    'La papelera lo guarda {days} días. Hasta entonces, Deshacer en la pantalla Medios lo devuelve a su elemento con su archivo.',
  'bulkDelete.set.checking': 'Comprobando lo que se va con este conjunto de elementos…',
  'bulkDelete.set.cannotCheck':
    'No se puede comprobar lo que se va con este conjunto de elementos, así que ahora no puede pasar a la papelera.',
  'bulkDelete.set.denied': 'No puede eliminar este conjunto de elementos.',
  'bulkDelete.set.trash':
    'La papelera lo guarda {days} días. Hasta entonces, Deshacer en la pantalla Conjuntos de elementos lo devuelve con sus elementos y los sitios que lo muestran.',
};
