/**
 * The Nepali item, item set and media page messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Nepali. Correct
 * a string here; a later run keeps it. It is typed `ResourcesMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { ResourcesMessages } from '../index';

export const ne: ResourcesMessages = {
  'itemSites.legend': 'साइटहरू',
  'itemSites.none': 'तपाईँ कुनै पनि साइटमा वस्तुहरू थप्न सक्नुहुन्न।',
  'itemSites.createHelp':
    'यदि तपाईँ कुनै साइट रोज्नुभयो भने, जसले आफ्नो खोजसँग सिङ्क गर्छ, त्यसले यो वस्तु थप्नेछ जब यो मेल खान्छ।',
  'loading.itemSet': 'वस्तु सेट लोड हुँदैछ...',
  'loading.media': 'मिडिया लोड हुँदैछ...',
  'values.none': 'कुनै मानहरू छैनन्।',
  'values.property': 'गुण',
  'values.type': 'प्रकार',
  'values.value': 'मान',
  'values.tableLabel': '{term} का लागि मानहरू',
  'field.resourceTemplate': 'संसाधन टेम्प्लेट',
  'column.modified': 'परिमार्जित',
  'items.add': 'वस्तु थप्नुहोस्',
  'items.empty': 'कुनै पनि वस्तु फेला परेन।',
  'items.mediaHeading': 'मिडिया ({count})',
  'items.noMedia': 'कुनै मिडिया छैन।',
  'items.valuesHeading': 'मानहरू',
  'items.rowName': 'वस्तु #{id}',
  'items.editLabel': 'यस वस्तुलाई सम्पादन गर्नुहोस्',
  'items.deleteLabel': 'यस वस्तुलाई मेटाउनुहोस्',
  'items.noSets': 'कुनै पनि वस्तु सेटहरू छैनन्।',
  'items.setsLegend': 'वस्तु सेटहरू',
  'itemSets.open': 'वस्तु सेट खोल्नुहोस्',
  'itemSets.add': 'वस्तु सेट थप्नुहोस्',
  'itemSets.deleteLabel': 'यस वस्तु सेटलाई मेटाउनुहोस्',
  'itemSets.rowName': 'वस्तु सेट #{id}',
  'media.legend': 'मिडिया',
  'media.altText': 'वैकल्पिक पाठ',
  'media.language': 'भाषा',
  'media.visibility': 'दृश्यता',
  'media.partOfItem': 'वस्तुको भाग',
  'media.mimeType': 'MIME प्रकार',
  'media.renderer': 'रेन्डरर',
  'media.ingester': 'इन्जेस्टर',
  'media.source': 'स्रोत',
  'media.filename': 'फाइलको नाम',
  'media.size': 'आकार',
  'media.sha256': 'SHA-256',
  'media.fileDerivatives': 'फाइल व्युत्पन्न',
  'media.editLabel': 'यो मिडिया सम्पादन गर्नुहोस्',
  'media.deleteLabel': 'यो मिडिया मेटाउनुहोस्',
  'media.noPreview': 'कुनै पूर्वावलोकन छैन',
  'media.altFallback': 'मिडिया #{id}',
  'route.invalidItemId': '"{param}" एक मान्य वस्तु id होइन।',
  'route.invalidItemSetId': '"{param}" एक मान्य वस्तु सेट id होइन।',
  'route.invalidMediaId': '"{param}" एक मान्य मिडिया id होइन।',
  'items.selectAll': 'यस पृष्ठमा सबै छान्नुहोस्',
  'items.selectItem': '{title} छान्नुहोस्',
  'items.selected': 'छानिएका: {count}',
  'items.bulkAdd': 'साइटमा थप्नुहोस्…',
  'items.bulkRemove': 'साइटबाट हटाउनुहोस्…',
  'items.bulkSite': 'साइट',
  'items.bulkChooseSite': 'साइट छान्नुहोस्',
  'items.bulkConfirmAdd': '{site} मा {count} थप्नुहोस्',
  'items.bulkDone': '{site} मा अहिले {total} वस्तुहरू छन्।',
  'items.bulkConfirmRemove': '{site} बाट {count} हटाउनुहोस्',
  'place.legend': 'नक्सामा राख्नुहोस्: {field}',
  'place.help':
    'नक्सामा क्लिक गर्नुहोस्, वा "lat, lng" टाइप गर्नुहोस्। प्रत्येक स्थान {term} मानको रूपमा सुरक्षित गरिन्छ, जसलाई साइटको नक्साले देखाउँछ।',
  'place.mapLabel': 'नक्सा: चयन गरिएको स्थान सेट गर्न क्लिक गर्नुहोस्',
  'place.value': '{number} स्थान',
  'place.remove': '{number} स्थान हटाउनुहोस्',
  'place.add': 'स्थान थप्नुहोस्',
  'place.invalid':
    '-90 देखि 90 सम्मको अक्षांश, एउटा अल्पविराम, र -180 देखि 180 सम्मको देशान्तर टाइप गर्नुहोस्।',
  'place.unavailable': 'नक्सा लोड हुन सकेन। स्थानलाई "lat, lng" को रूपमा टाइप गर्नुहोस्।',
};
