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
  'bulkDelete.empty.askAll':
    'ट्रॅशमधील सर्व काही कायमचे हटवले जाते: मीडिया आणि फाईल्ससह आयटम, फाईल्ससह मीडिया आणि आयटम सेट्स. हे हटवणे आता पूर्ववत करता येणार नाही.',
  'bulkDelete.empty.confirm': 'कचरा रिकामी करा',
  'bulkDelete.empty.cancel': 'रद्द करा',
  'bulkDelete.empty.emptied': 'ट्रॅश रिकामी आहे.',
  'bulkDelete.empty.itemsOne': '1 आयटम कायमचा हटवला.',
  'bulkDelete.empty.itemsMany': '{count} आयटम कायमचे हटवले.',
  'bulkDelete.empty.mediaOne': '1 मीडिया कायमचा हटवला.',
  'bulkDelete.empty.mediaMany': '{count} मीडिया कायमचे हटवले.',
  'bulkDelete.empty.setsOne': '1 आयटम सेट कायमचा हटवला.',
  'bulkDelete.empty.setsMany': '{count} आयटम सेट्स कायमचे हटवले.',
  'bulkDelete.empty.nothing': 'कचरा आधीच रिकामी होता.',
  'bulkDelete.item.checking': 'या बाबीसोबत काय येते ते तपासत आहे…',
  'bulkDelete.item.cannotCheck':
    'या बाबीसोबत काय येते ते तपासता येत नाही, त्यामुळे आता ती ट्रॅशमध्ये हलवता येत नाही.',
  'bulkDelete.item.checkAgain': 'पुन्हा तपासा',
  'bulkDelete.item.denied': 'तुम्ही ही बाबी हटवू शकत नाही.',
  'bulkDelete.item.ask': '{name} ला ट्रॅशमध्ये हलवा?',
  'bulkDelete.item.linked': 'इतर नोंदी याला जोडलेल्या आहेत:',
  'bulkDelete.item.linksBack': 'तुम्ही ते पूर्ववत केल्यास त्यांच्या लिंक्स पुन्हा येतील.',
  'bulkDelete.item.notLinked': 'याला कोणतीही लिंक जोडलेली नाही.',
  'bulkDelete.item.trash':
    'ट्रॅश {days} दिवसांसाठी ते राखून ठेवते. ते पूर्ववत केल्यास, Items screen वरील Undo मुळे मीडिया आणि फाईल्ससह ते पुन्हा पूर्ववत होते.',
  'bulkDelete.item.move': 'ट्रॅशमध्ये हलवा',
  'bulkDelete.item.moving': 'ट्रॅशमध्ये हलवत आहे…',
  'bulkDelete.item.failed': '{name} ट्रॅशमध्ये हलवले गेले नाही. ते पूर्वीप्रमाणेच राहते.',
  'bulkDelete.item.trashed': '{name} हटवले.',
  'bulkDelete.item.undo': 'पूर्ववत करा',
  'bulkDelete.item.undoing': '{name} परत ठेवत आहे…',
  'bulkDelete.item.back': '{name} परत आले.',
  'bulkDelete.item.notBack': '{name} ट्रॅशमध्येच राहते.',
  'bulkDelete.media.checking': 'या मीडियासह काय येते ते तपासत आहे…',
  'bulkDelete.media.cannotCheck':
    'या मीडियासह काय येते ते तपासता येत नाही, म्हणून आता ते ट्रॅशमध्ये हलवता येत नाही.',
  'bulkDelete.media.denied': 'तुम्ही हा मीडिया हटवू शकत नाही.',
  'bulkDelete.media.file': 'याची फाईल त्याच्यासोबत जाते: {size}.',
  'bulkDelete.media.trash':
    'कचरा {days} दिवसांसाठी ते ठेवतो. तोपर्यंत, मीडिया स्क्रीनवरील अनडू (Undo) मुळे ते त्याच्या फाईलसह त्याच्या आयटमवर परत येते.',
  'bulkDelete.set.checking': 'या आयटम सेटसह काय येते ते तपासत आहे…',
  'bulkDelete.set.cannotCheck':
    'या आयटम सेटसह काय येते ते तपासता येत नाही, म्हणून आता ते ट्रॅशमध्ये हलवता येत नाही.',
  'bulkDelete.set.denied': 'तुम्ही हा आयटम सेट हटवू शकत नाही.',
  'bulkDelete.set.trash':
    'कचरा {days} दिवसांसाठी ते ठेवतो. तोपर्यंत, आयटम सेट स्क्रीनवरील अनडू (Undo) मुळे ते त्याच्या आयटमसह आणि ते दर्शवणाऱ्या साइट्ससह पुन्हा एकत्र येते.',
};
