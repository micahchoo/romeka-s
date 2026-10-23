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
  'termLists.deleteLabel': 'यो शब्द सूची मेटाउनुहोस्',
  'termLists.deleteNote':
    'सूची मेटाउँदा यसबाट छानिएका मानहरू हट्छन्। ती मानहरू तिनीहरूका वस्तुहरूमा साधारण पाठको रूपमा रहन्छन्।',
  'termPicker.loading': 'सूची लोड हुँदैछ...',
  'termPicker.noMatch': 'कुनै पनि प्रविष्टि "{query} सँग मेल खाँदैन।',
  'termPicker.notInList':
    '"{value}" यस सूचीमा छैन। यसलाई प्रतिस्थापन गर्न एउटा प्रविष्टि छान्नुहोस्।',
  'termPicker.hint': '{list} सूची फिल्टर गर्न टाइप गर्नुहोस्, त्यसपछि एउटा प्रविष्टि छान्नुहोस्।',
  'termPicker.typeLabel': 'शब्द सूची: {list}',
  'tags.title': 'ट्यागहरू',
  'tags.intro':
    'वर्णन गर्दा मानिसहरूले थपेका स्वतन्त्र शब्दहरू। जब केही मानिसहरू बारम्बार आउँछन्, तिनीहरूलाई सूचीमा बदल्नुहोस्।',
  'tags.empty':
    'अहिलेसम्म कुनै ट्यागहरू छैनन्। तिनीहरूलाई कुनै वस्तु, वस्तु सेट वा मिडियाको ट्याग क्षेत्रमा थप्नुहोस्।',
  'termLists.tagsLegend': 'प्रवर्द्धन गर्नुपर्ने ट्यागहरू',
  'tags.listName': 'सूचीको नाम',
  'tags.listHint': 'शब्दहरूको सूची, वा तपाईँले थप्न चाहनुभएको ट्यागहरूको सूची।',
  'tags.promote': 'सूचीमा प्रवर्द्धन गर्नुहोस्',
  'tags.promoteTitle': '“{list}” मा प्रवर्द्धन गर्नुहोस्',
  'tags.newList': 'यसले नयाँ सूची बनाउँछ।',
  'tags.fields': '"विषय" क्षेत्रमा सूची प्रदान गर्ने वस्तुका प्रकारहरू',
  'tags.fieldsSkipped': 'तपाईँले सम्पादन गर्न नसक्ने वस्तुहरू, जसलाई यथावत छोडिएको छ',
  'tags.skipped': 'तपाईँले सम्पादन गर्न नसक्ने स्रोतहरू, जसमा ट्यागहरू राखिएका छन्',
  'tags.promoted': '“{list}” मा प्रवर्द्धन गरियो। स्रोतहरू परिवर्तन भए: {changed}।',
  'rewrite.changes': 'परिवर्तन हुने स्रोतहरू',
  'rewrite.skipped': 'तपाईँले सम्पादन गर्न नसक्ने स्रोतहरू, जसले आफ्नो हिज्जे कायम राख्छन्',
  'rewrite.none': 'कुनै पनि छैन',
  'merge.title': 'समान हिज्जेहरू',
  'merge.intro':
    'यी एउटै मान जस्ता देखिन्छन्। तिनीहरूलाई मिसाउनुहोस् ताकि एउटा खोज्दा ती सबै फेला परून्।',
  'merge.keep': '“{value}” लाई राख्नुहोस्',
  'merge.usedBy': '{count} द्वारा प्रयोग गरिएको',
  'merge.into': '“{value}” मा मिसाउनुहोस्',
  'merge.confirm': 'मिसाउनुहोस्',
  'merge.different': 'यी फरक छन्',
  'merge.merged': '“{value}” मा मिसाइएको। स्रोतहरू परिवर्तन भए: {changed}।',
  'merge.remembered': 'बुझियो। यी हिज्जेहरूलाई फेरि मिसाउनका लागि प्रस्ताव गरिने छैन।',
  'merge.deprecates':
    'यो सूची प्रकाशित छ, र प्रकाशित शब्दले यसलाई कहिल्यै छोड्दैन। {values} deprecated नै रहनेछन्, “{value}” द्वारा प्रतिस्थापित हुनेछन्, ताकि यसका लिङ्कहरूले काम गरिरहून्। यसलाई सार्वजनिक बनाउन सूचीलाई फेरि प्रकाशित गर्नुहोस्।',
  'termPicker.deprecated':
    '“{value}” यो सूचीमा deprecated छ। जबसम्म तपाईँले हालको प्रविष्टि छान्नुहुन्न, यो यस वस्तुमा रहनेछ।',
};
