/**
 * The Marathi bulk delete messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Marathi. Correct
 * a string here; a later run keeps it. It is typed `BulkDeleteMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { BulkDeleteMessages } from '../index';

export const mr: BulkDeleteMessages = {
  'bulkDelete.scope.refused':
    'तुमची भूमिका केवळ चिन्हांकित केलेल्या बाबी हटवू शकते. हटवण्यासाठी बाबींना चिन्हांकित करा.',
  'bulkDelete.ask.note':
    'पूर्वावलोकन काय जाते ते दर्शवते. जोपर्यंत तुम्ही पुष्टी करत नाही, तोपर्यंत काहीही कचऱ्यात जात नाही.',
  'bulkDelete.confirm.itemsOne': '1 बाब कचऱ्यात (ट्रॅश) जाते.',
  'bulkDelete.confirm.itemsMany': '{count} बाबी कचऱ्यात (ट्रॅश) जातात.',
  'bulkDelete.confirm.none': 'या बाबींपैकी कोणतीही एक तुम्ही हटवू शकता अशी नाही.',
  'bulkDelete.confirm.mediaOne': '1 मीडिया फाईल देखील: {size} जाते.',
  'bulkDelete.confirm.mediaMany': '{count} मीडिया फाईल्स एकूण {size} मध्ये जातात.',
  'bulkDelete.confirm.deniedOne': 'तुम्ही हटवू शकत नाही अशी 1 बाब तशीच राहते.',
  'bulkDelete.confirm.deniedMany': '{count} बाबी तुम्ही हटवू शकत नाही अशा तशाच राहतात.',
  'bulkDelete.confirm.samples': 'ज्या बाबी प्रथम जातात',
  'bulkDelete.confirm.trash':
    'ते {days} दिवसांसाठी कचऱ्यात राहतात. तोपर्यंत, अनडू (Undo) त्यांना त्यांच्या मीडिया आणि फाईल्ससह परत आणते.',
  'bulkDelete.confirm.type': 'पुष्टी करण्यासाठी, {count} बाबींची संख्या टाईप करा',
  'bulkDelete.confirm.moveOne': '1 बाब कचऱ्यात हलवा',
  'bulkDelete.confirm.moveMany': '{count} बाबी कचऱ्यात हलवा',
  'bulkDelete.empty.open': 'कचरा रिकामी करा…',
  'bulkDelete.empty.ask':
    'बल्क डिलीटद्वारे ट्रॅशमध्ये हलवलेल्या प्रत्येक बाबीची मीडिया आणि फाईल्ससह कायमची विल्हेवाट लावली जाते. या बदलांची आता पूर्ववत (Undo) करता येणार नाही.',
  'bulkDelete.empty.confirm': 'कचरा रिकामी करा',
  'bulkDelete.empty.cancel': 'रद्द करा',
  'bulkDelete.empty.doneOne': 'कचरा रिकामी झाला. 1 बाबीची कायमची विल्हेवाट लावली.',
  'bulkDelete.empty.done': 'कचरा रिकामी झाला. {count} बाबींची कायमची विल्हेवाट लावली.',
  'bulkDelete.empty.nothing': 'कचरा आधीच रिकामी होता.',
};
