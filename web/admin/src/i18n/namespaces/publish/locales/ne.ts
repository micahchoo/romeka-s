/**
 * The Nepali Publish panel messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Nepali. Correct
 * a string here; a later run keeps it. It is typed `PublishMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { PublishMessages } from '../index';

export const ne: PublishMessages = {
  'publish.heading': 'प्रकाशित गर्नुहोस्',
  'publish.ledeTermList':
    'प्रकाशनले यस सूची र यसका प्रत्येक शब्दलाई एउटा स्थायी वेब ठेगाना प्रदान गर्दछ, जसलाई मानिस र प्रोग्रामहरूले पढ्न सक्छन्।',
  'publish.ledeVocabulary':
    'प्रकाशनले यो मानक र यसका प्रत्येक शब्दलाई एउटा स्थायी वेब ठेगाना प्रदान गर्दछ, जसलाई मानिस र प्रोग्रामहरूले पढ्न सक्छन्।',
  'publish.loading': 'प्रकाशन लोड हुँदैछ...',
  'publish.stateDraft': 'मस्यौदा। अहिलेसम्म प्रकाशित भएको छैन।',
  'publish.statePublished': 'प्रकाशित। संस्करण {version}, {date}।',
  'publish.publicUrl': 'सार्वजनिक ठेगाना',
  'publish.promise':
    'एक पटक प्रकाशित भएपछि, शब्दहरू थप्न वा हटाउन सकिन्छ, तर कहिल्यै हटाउन वा नाम परिवर्तन गर्न सकिँदैन, र ठेगाना कहिल्यै परिवर्तन हुँदैन। अन्य अभिलेखहरूले पहिले नै तिनीहरूलाई लिङ्क गरिसकेका हुन सक्छन्।',
  'publish.slugLabel': 'ठेगानाको नाम',
  'publish.slugHint': 'सानो अक्षर, अङ्क र एउटा हाइफन। यसलाई {url} मा सेवा दिइन्छ',
  'publish.slugInvalid': 'सानो अक्षर, अङ्क र एउटा हाइफन प्रयोग गर्नुहोस्, 64 अक्षरसम्म',
  'publish.namespaceLegend': 'शब्दहरूको ठेगाना',
  'publish.namespaceArchive': 'यस अभिलेखको ठेगाना: {url}',
  'publish.namespacePersistent': 'w3id.org वा purl.org मा निरन्तर ठेगाना',
  'publish.namespaceWhy':
    'निरन्तर ठेगानाले अर्को डोमेनमा स्थानान्तरण भए तापनि टिक्छ: सेवाले यसलाई यहाँ रिडिरेक्ट गर्दछ, त्यसैले तपाईँका शब्दहरूका लिङ्कहरूले काम गरिरहनेछन् यदि यो अभिलेख स्थानान्तरण भएमा।',
  'publish.namespaceLabel': 'निरन्तर नामस्थान',
  'publish.namespaceHint':
    'उदाहरणका लागि https://w3id.org/your-archive/places/. यसलाई / वा # मा समाप्त गर्नुहोस्।',
  'publish.namespaceInvalid':
    'w3id.org वा purl.org मा एउटा ठेगाना प्रयोग गर्नुहोस् जुन / वा # मा समाप्त हुन्छ।',
  'publish.redirectHelp':
    'त्यसपछि सेवालाई यहाँ रिडिरेक्ट गर्न लगाउनुहोस्, उदाहरणका लागि यस w3id.org नियमको साथ:',
  'publish.namespace': 'नामस्थान',
  'publish.termsHeading': 'शब्दहरू',
  'publish.columnTerm': 'शब्द',
  'publish.columnUri': 'URI',
  'publish.columnState': 'अवस्था',
  'publish.columnChange': 'परिवर्तन',
  'publish.termDraft': 'अहिलेसम्म प्रकाशित नभएको',
  'publish.termPublished': 'प्रकाशित',
  'publish.termDeprecated': 'अप्रचलित',
  'publish.termDeprecationPending': 'आगामी संस्करणबाट अप्रचलित',
  'publish.deprecate': 'अप्रचलित गर्ने',
  'publish.deprecateLabel': '{term} लाई अप्रचलित गर्ने',
  'publish.undeprecate': 'राख्ने',
  'publish.undeprecateLabel': '{term} लाई प्रयोगमा राख्ने',
  'publish.links': 'लिङ्कहरू',
  'publish.linksLabel': '{term} का लिङ्कहरू कुनै आधिकारिक निकायसँग',
  'publish.exactMatch': 'अन्यत्र एउटै शब्द (exactMatch)',
  'publish.closeMatch': 'अन्यत्र नजिकको शब्द (closeMatch)',
  'publish.linksHint': 'GeoNames वा Wikidata जस्तो कुनै आधिकारिक निकायमा यस शब्दको ठेगाना।',
  'publish.saveLinks': 'लिङ्कहरू सुरक्षित गर्ने',
  'publish.matchInvalid': 'http:// वा https:// बाट सुरु हुने ठेगाना प्रयोग गर्नुहोस्।',
  'publish.publish': 'प्रकाशित गर्ने',
  'publish.publishVersion': 'संस्करण {version} प्रकाशित गर्ने',
  'publish.noChanges': 'संस्करण {version} देखि केही परिवर्तन भएको छैन।',
  'publish.pending':
    'संस्करण {version} पछिका परिवर्तनहरू अहिलेसम्म सार्वजनिक छैनन्। तिनीहरूलाई थप्नका लागि प्रकाशित गर्नुहोस्।',
  'publish.published': 'संस्करण {version} प्रकाशित भयो।',
  'publish.versionsHeading': 'संस्करणहरू',
  'publish.versionLine': 'संस्करण {version}, {date}, {terms} शब्दहरू',
  'publish.previewHeading': 'टर्टलमा पूर्वावलोकन',
  'publish.replacedBy': '“{term}” द्वारा प्रतिस्थापित',
};
