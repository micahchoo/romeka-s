/**
 * The Nepali Term lists messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Nepali. Correct
 * a string here; a later run keeps it. It is typed `TermListMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { TermListMessages } from '../index';

export const ne: TermListMessages = {
  'termLists.resource': 'शब्द सूचीहरू',
  'termLists.invalidId': '"{param}" शब्द सूचीको id होइन।',
  'termLists.intro':
    'एउटा शब्द सूचीले क्षेत्रलाई निश्चित प्रविष्टिहरूको सेटबाट छनोटमा बदल्छ, ताकि एउटै स्थान वा विषय सधैँ एउटै तरिकाले हिज्जे गरिन्छ।',
  'termLists.add': 'शब्द सूची थप्नुहोस्',
  'termLists.legend': 'शब्द सूची',
  'termLists.kindLegend': 'सूचीले के राख्छ',
  'termLists.kindTerms': 'शब्दहरूको सूची',
  'termLists.kindItems': 'वस्तुहरू (item set) बाट वस्तुहरू',
  'termLists.kindUris': 'लेबलहरू सहितका लिङ्कहरू (URIs)',
  'termLists.kindEmpty': 'अहिलेसम्म केही छैन',
  'termLists.termsLabel': 'प्रति लाइन एउटा शब्द',
  'termLists.urisLabel': 'प्रति लाइन एउटा लिङ्क: URI, एउटा खाली ठाउँ, त्यसपछि लेबल',
  'termLists.itemSetLabel': 'वस्तु सेट',
  'termLists.selectItemSet': 'एउटा वस्तु सेट चयन गर्नुहोस्',
  'termLists.langLabel': 'शब्दहरूको भाषा',
  'termLists.langHint':
    'वैकल्पिक। सूचीबाट छानिएको प्रत्येक शब्दले यो भाषा ट्याग बोक्छ, उदाहरणका लागि en।',
  'termLists.errorNoEntries': 'कम्तीमा एउटा प्रविष्टि थप्नुहोस्।',
  'termLists.errorNoItemSet': 'एउटा वस्तु सेट चयन गर्नुहोस्।',
  'termLists.columnKind': 'धारण गर्ने',
  'termLists.columnEntries': 'प्रविष्टिहरू',
  'termLists.columnUsage': 'यसको प्रयोग गर्ने वस्तुहरू',
  'termLists.editLabel': 'यो शब्द सूची सम्पादन गर्नुहोस्',
  'termLists.deleteLabel': 'यो शब्द सूची मेटाउनुहोस्',
  'termLists.deleteNote':
    'सूची मेटाउँदा यसबाट छानिएका मानहरू हट्छन्। ती मानहरू तिनीहरूका वस्तुहरूमा साधारण पाठको रूपमा रहन्छन्।',
  'termPicker.loading': 'सूची लोड हुँदैछ...',
  'termPicker.noMatch': 'कुनै पनि प्रविष्टि "{query} सँग मेल खाँदैन।',
  'termPicker.notInList':
    '"{value}" यस सूचीमा छैन। यसलाई प्रतिस्थापन गर्न एउटा प्रविष्टि छान्नुहोस्।',
  'termPicker.hint': '{list} सूची फिल्टर गर्न टाइप गर्नुहोस्, त्यसपछि एउटा प्रविष्टि छान्नुहोस्।',
  'termPicker.typeLabel': 'शब्द सूची: {list}',
};
