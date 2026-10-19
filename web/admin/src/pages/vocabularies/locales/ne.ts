/**
 * The Nepali vocabulary import messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Nepali. Correct
 * a string here; a later run keeps it. It is typed `ImportMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { ImportMessages } from '../messages';

export const ne: ImportMessages = {
  'import.heading': 'शब्दावली आयात गर्नुहोस्',
  'import.lede':
    'RDF फाइल वा वेब ठेगानाबाट प्रकाशित शब्दावली ल्याउनुहोस्। यसका वर्गहरू र गुणहरू तपाईँका क्षेत्र र वस्तु प्रकारहरूमा उपलब्ध हुन्छन्।',
  'import.submit': 'शब्दावली आयात गर्नुहोस्',
  'import.working': 'शब्दावली पढ्दै…',
  'source.legend': 'शब्दावली कहाँबाट आउँछ',
  'source.upload': 'यो कम्प्युटरमा रहेको एउटा फाइल',
  'source.url': 'एउटा वेब ठेगाना',
  'source.file': 'शब्दावली फाइल',
  'source.urlLabel': 'फाइलको वेब ठेगाना',
  'source.format': 'फाइल ढाँचा',
  'source.formatHelp':
    'जबसम्म आयातले ढाँचा बताउन सक्दैन, तबसम्म यसलाई “स्वचालित रूपमा पत्ता लगाउनुहोस्” मा छोड्नुहोस्।',
  'format.guess': 'स्वचालित रूपमा पत्ता लगाउनुहोस्',
  'format.rdfxml': 'RDF/XML (.rdf, .owl)',
  'format.turtle': 'टर्टल वा नोटेसन3 (.ttl, .n3)',
  'format.ntriples': 'एन-ट्रिपल्स (.nt)',
  'format.jsonld': 'JSON-LD (.jsonld)',
  'about.legend': 'शब्दावलीको बारेमा',
  'about.namespace': 'नेमस्पेस URI',
  'about.namespaceHelp':
    'शब्दावलीको प्रत्येक वर्ग र गुणले सुरु गर्ने वेब ठेगाना, जस्तै http://xmlns.com/foaf/0.1/. यसको मुनि रहेका शब्दहरू मात्र आयात गरिन्छ। यो सामान्यतया / वा # मा समाप्त हुन्छ।',
  'about.prefix': 'उपनाम',
  'about.prefixHelp':
    'प्रत्येक शब्द अगाडि लेखिने छोटो नाम, जस्तै foaf:name। अक्षर, सङ्ख्या, हाइफन र अन्डरस्कोर प्रयोग गर्नुहोस्।',
  'about.label': 'लेबल',
  'about.labelHelp': 'यस शब्दावलीका लागि मानिसहरूले देख्ने नाम।',
  'about.comment': 'विवरण (वैकल्पिक)',
  'advanced.summary': 'उन्नत',
  'advanced.lang': 'प्राथमिकता भाषा',
  'advanced.langHelp':
    'English वा fr जस्तो भाषा ट्याग। त्यस भाषाका लेबल र टिप्पणीहरू राखिन्छन्। यदि छैन भने, वा कुनै शब्दमा नभएको खण्डमा, फाइलको पहिलो शब्द राखिन्छ।',
  'advanced.label': 'लेबल प्रोपर्टी',
  'advanced.labelHelp':
    'असामान्य प्रोपर्टीले नामकरण गर्ने शब्दावलीका लागि मात्र। पूर्ण प्रोपर्टी URI लाई < > मा लेख्नुहोस्, जस्तै <http://schema.org/name>। बिना, skos:prefLabel, rdfs:label, foaf:name, rss:title र dc:title लाई पालैपालो प्रयास गरिन्छ।',
  'advanced.comment': 'टिप्पणी प्रोपर्टी',
  'advanced.commentHelp': 'प्रत्येक शब्दको विवरणका लागि, rdfs:comment पढिन्छ।',
  'error.noFile': 'आयात गर्नका लागि एउटा फाइल छान्नुहोस्।',
  'error.noUrl': 'फाइलको वेब ठेगाना प्रविष्ट गर्नुहोस्।',
  'update.toggle': 'फाइल वा वेब ठेगानाबाट अपडेट गर्नुहोस्',
  'update.lede':
    'यस शब्दावलीलाई यसको फाइलको नयाँ प्रतिलिपिसँग तुलना गर्नुहोस्। तपाईँले केही पनि सुरक्षित गर्नु अघि हरेक परिवर्तन देख्नुहुन्छ।',
  'update.compare': 'तुलना',
  'update.comparing': 'तुलना गर्दै…',
  'diff.review': 'लागू गर्नु अघि यी परिवर्तनहरूको समीक्षा गर्नुहोस्।',
  'diff.none': 'यो शब्दावलीले पहिले नै फाइलसँग मेल खान्छ। परिवर्तन गर्नुपर्ने केही छैन।',
  'diff.newClasses': 'नयाँ वर्गहरू ({count})',
  'diff.newProperties': 'नयाँ गुणहरू ({count})',
  'diff.changed': 'परिवर्तन गरिएका लेबल र टिप्पणीहरू ({count})',
  'diff.removed': 'फाइलबाट हटाइएको ({count})',
  'diff.removedNote':
    'यी अभिलेखमा रहन्छन्, र तिनीहरूलाई प्रयोग गर्ने मानहरू र वस्तुका प्रकारहरू पनि रहन्छन्। यदि अब आवश्यक छैन भने, आफ्नै पृष्ठबाट एउटा हटाउनुहोस्।',
  'diff.term': 'शब्द',
  'diff.label': 'लेबल',
  'diff.comment': 'टिप्पणी',
  'diff.what': 'के परिवर्तन भयो',
  'diff.now': 'अहिले',
  'diff.file': 'फाइलमा',
  'diff.blank': '(केही छैन)',
  'diff.apply': 'यी परिवर्तनहरू लागू गर्नुहोस्',
  'diff.applying': 'लागू गर्दै…',
  'diff.cancel': 'रद्द',
  'diff.applied': 'परिवर्तनहरू सुरक्षित गरियो।',
};
