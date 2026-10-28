/**
 * The Tamil item history messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Tamil. Correct
 * a string here; a later run keeps it. It is typed `ItemHistoryMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { ItemHistoryMessages } from '../index';

export const ta: ItemHistoryMessages = {
  'itemHistory.loading': 'வரலாற்றை வாசித்தல்…',
  'itemHistory.empty': 'இந்த உருப்படியின் மாற்றங்கள் எதுவும் இன்னும் பதிவு செய்யப்படவில்லை.',
  'itemHistory.entry': '{date} · {who}',
  'itemHistory.entrySource': '{date} · {who} · {source}',
  'itemHistory.deletedUser': 'நீக்கப்பட்ட பயனர்',
  'itemHistory.source.bulk': 'மொத்த மாற்றம் {id}',
  'itemHistory.source.import': 'இறக்குமதி {id}',
  'itemHistory.source.bulkUndo': 'மொத்த மாற்றம் {id}: மீட்பு',
  'itemHistory.source.importUndo': 'இறக்குமதி {id}: மீட்பு',
  'itemHistory.change': '{field}: {before} → {after}',
  'itemHistory.field.visibility': 'தெரிவுநிலை',
  'itemHistory.field.class': 'வகுப்பு',
  'itemHistory.field.template': 'உருப்படி வகை',
  'itemHistory.field.itemSets': 'உருப்படித் தொகுப்புகள்',
  'itemHistory.field.sites': 'தளங்கள்',
  'itemHistory.public': 'பொது',
  'itemHistory.private': 'தனிப்பட்ட',
  'itemHistory.none': '(ஏதுமில்லை)',
  'itemHistory.untitled': 'பெயரிடப்படாத {id}',
  'itemHistory.privateValue': '{value} (தனிப்பட்ட)',
  'itemHistory.putBack': 'இந்த மதிப்பை மீண்டும் உள்ளிடவும்',
  'itemHistory.puttingBack': 'மதிப்பை மீண்டும் உள்ளிடுகிறோம்…',
  'itemHistory.putBackDone':
    'மதிப்பு மீண்டும் கிடைத்துள்ளது. மேலே உள்ள புதிய மாற்றம் அதை உறுதிப்படுத்துகிறது.',
  'itemHistory.putBackSame': 'எதுவும் மாறவில்லை: அந்த உருப்படியில் இப்போது இந்த மதிப்பு உள்ளது.',
  'itemHistory.older': 'பழைய மாற்றங்களைக் காட்டு',
};
