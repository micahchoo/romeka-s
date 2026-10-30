/**
 * The Marathi item history messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Marathi. Correct
 * a string here; a later run keeps it. It is typed `ItemHistoryMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { ItemHistoryMessages } from '../index';

export const mr: ItemHistoryMessages = {
  'itemHistory.loading': 'इतिहासाचे वाचन…',
  'itemHistory.empty': 'या घटकात अद्याप कोणताही बदल नोंदवलेला नाही.',
  'itemHistory.entry': '{date} · {who}',
  'itemHistory.entrySource': '{date} · {who} · {source}',
  'itemHistory.deletedUser': 'एक हटवलेला वापरकर्ता',
  'itemHistory.source.bulk': 'मोठ्या प्रमाणावर बदल {id}',
  'itemHistory.source.import': 'आयात {id}',
  'itemHistory.source.bulkUndo': 'मोठ्या प्रमाणावर बदल {id} पूर्ववत करणे',
  'itemHistory.source.importUndo': 'आयात {id} पूर्ववत करणे',
  'itemHistory.change': '{field}: {before} → {after}',
  'itemHistory.field.visibility': 'दृश्यमानता',
  'itemHistory.field.class': 'वर्ग',
  'itemHistory.field.template': 'घटकाचा प्रकार',
  'itemHistory.field.itemSets': 'घटकांचे संच',
  'itemHistory.field.sites': 'संकेतस्थळे',
  'itemHistory.public': 'सार्वजनिक',
  'itemHistory.private': 'खाजगी',
  'itemHistory.none': '(काहीही नाही)',
  'itemHistory.inTrash': '{name} (ट्रॅशमध्ये)',
  'itemHistory.deletedSet': 'हटवलेला वस्तू संच {id}',
  'itemHistory.privateValue': '{value} (खाजगी)',
  'itemHistory.putBack': 'हे मूल्य पुन्हा ठेवा',
  'itemHistory.puttingBack': 'मूल्य पुन्हा ठेवले जात आहे…',
  'itemHistory.putBackDone': 'मूल्य परत आले आहे. सर्वात वरचा नवीन बदल तेच सांगतो.',
  'itemHistory.putBackSame': 'काहीही बदलले नाही: या घटकाकडे आता हेच मूल्य आहे.',
  'itemHistory.older': 'जुने बदल दर्शवा',
};
