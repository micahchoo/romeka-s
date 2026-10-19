/**
 * The Hindi Term lists messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Hindi. Correct
 * a string here; a later run keeps it. It is typed `TermListMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { TermListMessages } from '../index';

export const hi: TermListMessages = {
  'termLists.resource': 'शब्द सूचियाँ',
  'termLists.invalidId': '"{param}" शब्द सूची की आईडी नहीं है।',
  'termLists.intro':
    'एक शब्द सूची किसी क्षेत्र या विषय को एक निश्चित सेट की प्रविष्टियों से विकल्प में बदल देती है, ताकि एक ही स्थान या विषय को हमेशा एक ही तरह से लिखा जाए।',
  'termLists.add': 'शब्द सूची जोड़ें',
  'termLists.legend': 'शब्द सूची',
  'termLists.kindLegend': 'सूची में क्या है',
  'termLists.kindTerms': 'शब्दों की सूची',
  'termLists.kindItems': 'आइटम सेट से आइटम',
  'termLists.kindUris': 'लेबल के साथ लिंक (URIs)',
  'termLists.kindEmpty': 'अभी कुछ नहीं',
  'termLists.termsLabel': 'प्रति पंक्ति एक शब्द',
  'termLists.urisLabel': 'लिंक, प्रति पंक्ति एक: URI, एक स्पेस, फिर लेबल',
  'termLists.itemSetLabel': 'आइटम सेट',
  'termLists.selectItemSet': 'आइटम सेट चुनें',
  'termLists.langLabel': 'शब्दों की भाषा',
  'termLists.langHint':
    'वैकल्पिक। सूची से चुने गए प्रत्येक शब्द में यह भाषा टैग होता है, उदाहरण के लिए en।',
  'termLists.errorNoEntries': 'कम से कम एक प्रविष्टि जोड़ें।',
  'termLists.errorNoItemSet': 'आइटम सेट चुनें।',
  'termLists.columnKind': 'होल्ड्स',
  'termLists.columnEntries': 'प्रविष्टियाँ',
  'termLists.columnUsage': 'इसका उपयोग करने वाले आइटम',
  'termLists.editLabel': 'इस शब्द सूची को संपादित करें',
  'termLists.deleteLabel': 'इस शब्द सूची को हटाएँ',
  'termLists.deleteNote':
    'सूची को हटाने से उसमें से चुने गए मान हट जाते हैं। वे उनके आइटम पर सादे टेक्स्ट के रूप में बने रहते हैं।',
  'termPicker.loading': 'सूची लोड हो रही है...',
  'termPicker.noMatch': 'कोई प्रविष्टि "{query}" से मेल नहीं खाती।',
  'termPicker.notInList': '"{value}" इस सूची में नहीं है। इसे बदलने के लिए एक प्रविष्टि चुनें।',
  'termPicker.hint': '{list} सूची को फ़िल्टर करने के लिए टाइप करें, फिर एक प्रविष्टि चुनें।',
  'termPicker.typeLabel': 'शब्द सूची: {list}',
};
