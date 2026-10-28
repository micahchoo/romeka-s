/**
 * The Nepali item history messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Nepali. Correct
 * a string here; a later run keeps it. It is typed `ItemHistoryMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { ItemHistoryMessages } from '../index';

export const ne: ItemHistoryMessages = {
  'itemHistory.loading': 'इतिहास पढ्दै…',
  'itemHistory.empty': 'यस वस्तुमा अहिलेसम्म कुनै परिवर्तन रेकर्ड गरिएको छैन।',
  'itemHistory.entry': '{date} · {who}',
  'itemHistory.entrySource': '{date} · {who} · {source}',
  'itemHistory.deletedUser': 'एउटा मेटाइएको प्रयोगकर्ता',
  'itemHistory.source.bulk': 'बल्क परिवर्तन {id}',
  'itemHistory.source.import': 'आयात {id}',
  'itemHistory.source.bulkUndo': 'बल्क परिवर्तन {id} को पूर्ववत अवस्था',
  'itemHistory.source.importUndo': 'आयात {id} को पूर्ववत अवस्था',
  'itemHistory.change': '{field}: {before} → {after}',
  'itemHistory.field.visibility': 'दृश्यता',
  'itemHistory.field.class': 'वर्ग',
  'itemHistory.field.template': 'वस्तुको प्रकार',
  'itemHistory.field.itemSets': 'वस्तु समूह',
  'itemHistory.field.sites': 'साइटहरू',
  'itemHistory.public': 'सार्वजनिक',
  'itemHistory.private': 'निजी',
  'itemHistory.none': '(छैन)',
  'itemHistory.untitled': 'शीर्षकविहीन {id}',
  'itemHistory.inTrash': '{name} (फोहोर टोकरीमा)',
  'itemHistory.deletedSet': 'मेटिएको वस्तु सेट {id}',
  'itemHistory.privateValue': '{value} (निजी)',
  'itemHistory.putBack': 'यो मान फिर्ता राख्नुहोस्',
  'itemHistory.puttingBack': 'मान फिर्ता राख्दै…',
  'itemHistory.putBackDone': 'मान फिर्ता भयो। सबैभन्दा नयाँ परिवर्तनले त्यही भन्छ।',
  'itemHistory.putBackSame': 'केही परिवर्तन भएन: वस्तुमा अहिले यो मान छ।',
  'itemHistory.older': 'पुराना परिवर्तनहरू देखाउनुहोस्',
};
