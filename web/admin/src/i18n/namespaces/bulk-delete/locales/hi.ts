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
  'bulkDelete.empty.ask':
    'बल्क डिलीट द्वारा ट्रैश में ले जाए गए प्रत्येक आइटम को उसके मीडिया और फ़ाइलों के साथ हमेशा के लिए हटा दिया जाता है। उन परिवर्तनों को अब अनडू नहीं किया जा सकता है।',
  'bulkDelete.empty.confirm': 'ट्रैश खाली करें',
  'bulkDelete.empty.cancel': 'रद्द करें',
  'bulkDelete.empty.doneOne': 'ट्रैश खाली है। 1 आइटम हमेशा के लिए हटा दिया गया।',
  'bulkDelete.empty.done': 'ट्रैश खाली है। {count} आइटम हमेशा के लिए हटा दिए गए।',
  'bulkDelete.empty.nothing': 'ट्रैश पहले से ही खाली है।',
};
