/**
 * The Hindi item history messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Hindi. Correct
 * a string here; a later run keeps it. It is typed `ItemHistoryMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { ItemHistoryMessages } from '../index';

export const hi: ItemHistoryMessages = {
  'itemHistory.loading': 'इतिहास पढ़ा जा रहा है…',
  'itemHistory.empty': 'इस आइटम में कोई बदलाव अभी तक दर्ज नहीं किया गया है।',
  'itemHistory.entry': '{date} · {who}',
  'itemHistory.entrySource': '{date} · {who} · {source}',
  'itemHistory.deletedUser': 'एक हटा हुआ उपयोगकर्ता',
  'itemHistory.source.bulk': 'थोक परिवर्तन {id}',
  'itemHistory.source.import': 'आयात {id}',
  'itemHistory.source.bulkUndo': 'थोक परिवर्तन {id} का पूर्ववत',
  'itemHistory.source.importUndo': 'आयात {id} का पूर्ववत',
  'itemHistory.change': '{field}: {before} → {after}',
  'itemHistory.field.visibility': 'दृश्यता',
  'itemHistory.field.class': 'वर्ग',
  'itemHistory.field.template': 'आइटम प्रकार',
  'itemHistory.field.itemSets': 'आइटम सेट',
  'itemHistory.field.sites': 'साइट्स',
  'itemHistory.public': 'सार्वजनिक',
  'itemHistory.private': 'निजी',
  'itemHistory.none': '(कोई नहीं)',
  'itemHistory.untitled': 'बिना नाम वाला {id}',
  'itemHistory.privateValue': '{value} (निजी)',
  'itemHistory.putBack': 'इस मान को वापस रखें',
  'itemHistory.puttingBack': 'मान को वापस रख रहे हैं…',
  'itemHistory.putBackDone': 'मान वापस आ गया है। सबसे ऊपर का नया बदलाव यही बताता है।',
  'itemHistory.putBackSame': 'कुछ नहीं बदला: आइटम में अब यह मान है।',
  'itemHistory.older': 'पुराने बदलाव दिखाएँ',
};
