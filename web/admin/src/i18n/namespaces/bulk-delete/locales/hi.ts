/**
 * The Hindi bulk delete messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Hindi. Correct
 * a string here; a later run keeps it. It is typed `BulkDeleteMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { BulkDeleteMessages } from '../index';

export const hi: BulkDeleteMessages = {
  'bulkDelete.scope.refused':
    'आपकी भूमिका केवल टिक किए गए आइटम को हटा सकती है। हटाने के लिए आइटम को टिक करें।',
  'bulkDelete.ask.note':
    'पूर्वावलोकन दिखाता है कि क्या जाता है। जब तक आप पुष्टि नहीं करते, कुछ भी ट्रैश में नहीं जाता।',
  'bulkDelete.confirm.itemsOne': '1 आइटम ट्रैश में जाता है।',
  'bulkDelete.confirm.itemsMany': '{count} आइटम ट्रैश में जाते हैं।',
  'bulkDelete.confirm.none': 'इनमें से कोई भी आइटम वह नहीं है जिसे आप हटा सकते हैं।',
  'bulkDelete.confirm.mediaOne': '1 मीडिया फ़ाइल भी जाती है: {size}।',
  'bulkDelete.confirm.mediaMany': '{count} मीडिया फ़ाइलें भी जाती हैं: कुल {size}।',
  'bulkDelete.confirm.deniedOne': '1 आइटम जिसे आप हटा नहीं सकते, वैसा ही रहता है।',
  'bulkDelete.confirm.deniedMany': '{count} आइटम जिन्हें आप हटा नहीं सकते, वे वैसे ही रहते हैं।',
  'bulkDelete.confirm.samples': 'वे पहले आइटम जो जाते हैं',
  'bulkDelete.confirm.trash':
    'वे {days} दिनों तक ट्रैश में रहते हैं। तब तक, अनडू उन्हें उनकी मीडिया और फ़ाइलों के साथ वापस डाल देता है।',
  'bulkDelete.confirm.type': 'पुष्टि करने के लिए आइटम की संख्या टाइप करें: {count}',
  'bulkDelete.confirm.moveOne': '1 आइटम को ट्रैश में ले जाएँ',
  'bulkDelete.confirm.moveMany': '{count} आइटम को ट्रैश में ले जाएँ',
  'bulkDelete.empty.open': 'ट्रैश खाली करें…',
  'bulkDelete.empty.askAll':
    'ट्रैश में मौजूद हर चीज़ हमेशा के लिए हटा दी जाती है: आइटम अपने मीडिया और फ़ाइलों के साथ, मीडिया अपनी फ़ाइल के साथ, और आइटम सेट। इन हटाने को अब पूर्ववत नहीं किया जा सकता।',
  'bulkDelete.empty.confirm': 'ट्रैश खाली करें',
  'bulkDelete.empty.cancel': 'रद्द करें',
  'bulkDelete.empty.emptied': 'ट्रैश खाली है।',
  'bulkDelete.empty.itemsOne': '1 आइटम को हमेशा के लिए हटा दिया गया।',
  'bulkDelete.empty.itemsMany': '{count} आइटम को हमेशा के लिए हटा दिया गया।',
  'bulkDelete.empty.mediaOne': '1 मीडिया को हमेशा के लिए हटा दिया गया।',
  'bulkDelete.empty.mediaMany': '{count} मीडिया को हमेशा के लिए हटा दिया गया।',
  'bulkDelete.empty.setsOne': '1 आइटम सेट को हमेशा के लिए हटा दिया गया।',
  'bulkDelete.empty.setsMany': '{count} आइटम सेट को हमेशा के लिए हटा दिया गया।',
  'bulkDelete.empty.nothing': 'ट्रैश पहले से ही खाली है।',
  'bulkDelete.item.checking': 'इस आइटम के साथ क्या जाता है इसकी जाँच की जा रही है…',
  'bulkDelete.item.cannotCheck':
    'इस आइटम के साथ क्या जाता है, इसकी जाँच नहीं की जा सकती, इसलिए इसे अभी ट्रैश में नहीं ले जाया जा सकता।',
  'bulkDelete.item.checkAgain': 'फिर से जाँचें',
  'bulkDelete.item.denied': 'आप इस आइटम को हटा नहीं सकते।',
  'bulkDelete.item.ask': '{name} को ट्रैश में ले जाएँ?',
  'bulkDelete.item.linked': 'अन्य रिकॉर्ड इससे जुड़े हैं:',
  'bulkDelete.item.linksBack': 'इसे पुनर्स्थापित करने पर उनके लिंक वापस आ जाते हैं।',
  'bulkDelete.item.notLinked': 'इससे कोई भी लिंक नहीं जुड़ता है।',
  'bulkDelete.item.trash':
    'ट्रैश इसे {days} दिनों के लिए रखता है। तब तक, आइटम्स स्क्रीन पर अनडू (Undo) करने से यह अपनी मीडिया और फाइलों के साथ वापस आ जाता है।',
  'bulkDelete.item.move': 'ट्रैश में ले जाएँ',
  'bulkDelete.item.moving': 'ट्रैश में ले जाया जा रहा है…',
  'bulkDelete.item.failed': '{name} को ट्रैश में नहीं ले जाया गया। यह वैसा ही रहता है।',
  'bulkDelete.item.trashed': '{name} को हटा दिया गया।',
  'bulkDelete.item.undo': 'पूर्ववत करें',
  'bulkDelete.item.undoing': '{name} को वापस रखा जा रहा है…',
  'bulkDelete.item.back': '{name} वापस आ गया है।',
  'bulkDelete.item.notBack': '{name} ट्रैश में ही रहता है।',
  'bulkDelete.media.checking': 'इस मीडिया के साथ क्या जाता है इसकी जाँच की जा रही है…',
  'bulkDelete.media.cannotCheck':
    'इस मीडिया के साथ क्या-क्या है, इसकी जाँच नहीं की जा सकती, इसलिए इसे अभी ट्रैश में नहीं ले जाया जा सकता।',
  'bulkDelete.media.denied': 'आप इस मीडिया को हटा नहीं सकते।',
  'bulkDelete.media.file': 'इसकी फ़ाइल इसके साथ जाती है: {size}।',
  'bulkDelete.media.trash':
    'ट्रैश इसे {days} दिनों के लिए रखता है। तब तक, मीडिया स्क्रीन पर अनडू (Undo) करने से यह अपनी आइटम और फ़ाइल के साथ वापस आ जाता है।',
  'bulkDelete.set.checking': 'इस आइटम सेट के साथ क्या जाता है इसकी जाँच की जा रही है…',
  'bulkDelete.set.cannotCheck':
    'इस आइटम सेट के साथ क्या-क्या है, इसकी जाँच नहीं की जा सकती, इसलिए इसे अभी ट्रैश में नहीं ले जाया जा सकता।',
  'bulkDelete.set.denied': 'आप इस आइटम सेट को हटा नहीं सकते।',
  'bulkDelete.set.trash':
    'ट्रैश इसे {days} दिनों के लिए रखता है। तब तक, आइटम सेट स्क्रीन पर अनडू (Undo) करने से यह अपनी आइटम्स और उन साइट्स के साथ वापस आ जाता है जो इसे दिखाते हैं।',
};
