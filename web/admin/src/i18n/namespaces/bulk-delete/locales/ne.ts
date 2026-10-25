/**
 * The Nepali bulk delete messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Nepali. Correct
 * a string here; a later run keeps it. It is typed `BulkDeleteMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { BulkDeleteMessages } from '../index';

export const ne: BulkDeleteMessages = {
  'bulkDelete.scope.refused':
    'तपाईँको भूमिकाले टिक गरिएका वस्तुहरू मात्र मेटाउन सक्छ। मेटाउनका लागि वस्तुहरूमा टिक लगाउनुहोस्।',
  'bulkDelete.ask.note':
    'पूर्वावलोकनले के जान्छ भन्ने देखाउँछ। जबसम्म तपाईँ पुष्टि गर्नुहुन्न, केही पनि फोहोरमा जाँदैन।',
  'bulkDelete.confirm.itemsOne': '1 वस्तु फोहोरमा जान्छ।',
  'bulkDelete.confirm.itemsMany': '{count} वस्तुहरू फोहोरमा जान्छन्।',
  'bulkDelete.confirm.none': 'यी मध्ये कुनै पनि वस्तु तपाईँले मेटाउन सक्ने वस्तु होइन।',
  'bulkDelete.confirm.mediaOne': '1 मिडिया फाइल पनि: {size}।',
  'bulkDelete.confirm.mediaMany': '{count} मिडिया फाइलहरू जम्मा {size} मा सबै जान्छन्।',
  'bulkDelete.confirm.deniedOne': '1 वस्तु जुन तपाईँ मेटाउन सक्नुहुन्न, त्यो यथावत रहन्छ।',
  'bulkDelete.confirm.deniedMany':
    '{count} वस्तुहरू जुन तपाईँ मेटाउन सक्नुहुन्न, ती यथावत रहन्छन्।',
  'bulkDelete.confirm.samples': 'जान्ने पहिलो वस्तुहरू',
  'bulkDelete.confirm.trash':
    'तिनीहरू {days} दिनसम्म फोहोरमा रहन्छन्। त्यसपछि, Undo ले तिनीहरूलाई तिनीहरूको मिडिया र फाइलहरू सहित फिर्ता ल्याउँछ।',
  'bulkDelete.confirm.type': 'पुष्टि गर्न, {count} वस्तुहरूको सङ्ख्या टाइप गर्नुहोस्',
  'bulkDelete.confirm.moveOne': '1 वस्तुलाई फोहोरमा सार्नुहोस्',
  'bulkDelete.confirm.moveMany': '{count} वस्तुहरूलाई फोहोरमा सार्नुहोस्',
  'bulkDelete.empty.open': 'फोहोर खाली गर्नुहोस्…',
  'bulkDelete.empty.ask':
    'बल्क डिलिट गरेर फोहोरमा सारिएका प्रत्येक वस्तुका लागि, यसको मिडिया र फाइलहरू सहित, यो स्थायी रूपमा मेटिन्छ। यस्ता परिवर्तनहरूलाई अब पूर्ववत गर्न सकिँदैन।',
  'bulkDelete.empty.confirm': 'फोहोर खाली गर्नुहोस्',
  'bulkDelete.empty.cancel': 'रद्द गर्नुहोस्',
  'bulkDelete.empty.doneOne': 'फोहोर खाली छ। 1 वस्तु स्थायी रूपमा मेटियो।',
  'bulkDelete.empty.done': 'फोहोर खाली छ। {count} वस्तुहरू स्थायी रूपमा मेटिए।',
  'bulkDelete.empty.nothing': 'फोहोर पहिले नै खाली थियो।',
};
