/**
 * The Hindi Standards and Fields page messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Hindi. Correct
 * a string here; a later run keeps it. It is typed `StandardsMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { StandardsMessages } from '../index';

export const hi: StandardsMessages = {
  'standards.lede':
    'वे साझा मानक जिनमें आपके फ़ील्ड और आइटम प्रकार सहेजे जाते हैं। आपको संग्रह को कैटलॉग करने के लिए उन्हें बदलने की आवश्यकता नहीं है।',
  'standards.define': '{term} का अर्थ',
  'standards.import': 'मानक आयात करें',
  'import.heading': 'मानक आयात करें',
  'import.lede':
    'RDF फ़ाइल या वेब पते से प्रकाशित मानक को लाएँ। इसकी क्लासेज और प्रॉपर्टीज़ आपके फ़ील्ड और आइटम प्रकारों के लिए उपलब्ध हो जाती हैं।',
  'import.submit': 'मानक आयात करें',
  'import.working': 'मानक पढ़ा जा रहा है…',
  'source.legend': 'मानक कहाँ से आता है',
  'source.upload': 'इस कंप्यूटर पर एक फ़ाइल',
  'source.url': 'एक वेब पता',
  'source.file': 'मानक फ़ाइल',
  'source.urlLabel': 'फ़ाइल का वेब पता',
  'source.format': 'फ़ाइल प्रारूप',
  'source.formatHelp':
    'जब तक आयात यह न कहे कि यह प्रारूप नहीं बता सकता, तब तक इसे “स्वचालित रूप से पता लगाएँ” पर छोड़ दें।',
  'format.guess': 'स्वचालित रूप से पता लगाएँ',
  'format.rdfxml': 'RDF/XML (.rdf, .owl)',
  'format.turtle': 'टर्टल या नोटेशन3 (.ttl, .n3)',
  'format.ntriples': 'N-ट्रिपल्स (.nt)',
  'format.jsonld': 'JSON-LD (.jsonld)',
  'about.legend': 'मानक के बारे में',
  'about.namespace': 'नेमस्पेस URI',
  'about.namespaceHelp':
    'मानक का वेब पता, जिससे प्रत्येक क्लास और प्रॉपर्टी शुरू होती है, जैसे http://xmlns.com/foaf/0.1/। इसके अंतर्गत केवल वे शब्द आयात किए जाते हैं जो इसके बाद आते हैं। यह आमतौर पर / या # पर समाप्त होता है।',
  'about.prefix': 'प्रीफ़िक्स',
  'about.prefixHelp':
    'प्रत्येक शब्द से पहले लिखी गई एक छोटी पहचान, जैसे foaf:name। अक्षरों, संख्याओं, हाइफ़न और अंडरस्कोर का उपयोग करें।',
  'about.label': 'लेबल',
  'about.labelHelp': 'इस मानक के लिए जो नाम लोग देखते हैं।',
  'about.comment': 'विवरण (वैकल्पिक)',
  'vocabularies.advancedSummary': 'उन्नत',
  'advanced.lang': 'पसंदीदा भाषा',
  'advanced.langHelp':
    'en या fr जैसी भाषा टैग। उस भाषा में लेबल और टिप्पणियाँ रखी जाती हैं। इसके बिना, या उस शब्द के लिए जिसमें यह नहीं है, फ़ाइल में पहला शब्द रखा जाता है।',
  'advanced.label': 'लेबल प्रॉपर्टी',
  'advanced.labelHelp':
    'केवल उस मानक के लिए जो अपने शब्दों के नाम किसी असामान्य प्रॉपर्टी से देता है। पूरा प्रॉपर्टी URI कोणीय कोष्ठक में लिखें, जैसे <http://schema.org/name>। इसके बिना, skos:prefLabel, rdfs:label, foaf:name, rss:title और dc:title को एक के बाद एक आज़माया जाता है।',
  'advanced.comment': 'टिप्पणी प्रॉपर्टी',
  'advanced.commentHelp':
    'प्रत्येक शब्द के विवरण के लिए, समान। इसके बिना, rdfs:comment पढ़ा जाता है।',
  'error.noFile': 'आयात करने के लिए एक फ़ाइल चुनें।',
  'error.noUrl': 'फ़ाइल का वेब पता दर्ज करें।',
  'update.toggle': 'फ़ाइल या वेब पते से अपडेट करें',
  'update.lede':
    'इस मानक की फ़ाइल की नई प्रति के साथ इसकी तुलना करें। आप कुछ भी सहेजे जाने से पहले हर बदलाव देखते हैं।',
  'update.compare': 'तुलना',
  'update.comparing': 'तुलना की जा रही है…',
  'diff.review': 'इन परिवर्तनों की समीक्षा करें, फिर उन्हें लागू करें।',
  'diff.none': 'यह मानक पहले से ही फ़ाइल से मेल खाता है। बदलने के लिए कुछ भी नहीं है।',
  'diff.newClasses': 'नई कक्षाएं ({count})',
  'diff.newProperties': 'नई विशेषताएं ({count})',
  'diff.changed': 'बदली गई लेबल और टिप्पणियां ({count})',
  'diff.removed': 'अब फ़ाइल में नहीं है ({count})',
  'diff.removedNote':
    'ये आर्काइव में रहेंगे, और उनके मानों तथा आइटम प्रकारों का उपयोग करने वाले आइटम भी रहेंगे। यदि अब उनकी आवश्यकता नहीं है, तो एक को उसकी अपनी पृष्ठ से हटा दें।',
  'diff.term': 'शब्द',
  'diff.label': 'लेबल',
  'diff.comment': 'टिप्पणी',
  'diff.what': 'क्या बदलाव',
  'diff.now': 'अब',
  'diff.file': 'फ़ाइल में',
  'diff.blank': '(कोई नहीं)',
  'diff.apply': 'इन परिवर्तनों को लागू करें',
  'diff.applying': 'लागू किया जा रहा है…',
  'diff.cancel': 'रद्द करें',
  'diff.applied': 'परिवर्तन सहेज लिए गए हैं।',
  'page.properties': 'गुण',
  'loading.vocabulary': 'मानक लोड हो रहा है...',
  'loading.property': 'प्रॉपर्टी लोड हो रही है...',
  'loading.resourceClass': 'क्लास लोड हो रही है...',
  'loading.vocabularyForm': 'मानक फॉर्म लोड हो रहा है...',
  'loading.propertyForm': 'प्रॉपर्टी फॉर्म लोड हो रहा है...',
  'loading.resourceClassForm': 'क्लास फॉर्म लोड हो रहा है...',
  'column.comment': 'टिप्पणी',
  'column.term': 'शब्द',
  'column.prefix': 'उपसर्ग',
  'column.namespace': 'नेमस्पेस',
  'column.vocabulary': 'मानक',
  'column.properties': 'गुण',
  'resource.properties': 'प्रॉपर्टीज',
  'resource.resourceClasses': 'क्लासेज़',
  'resource.vocabularies': 'मानक',
  'detail.namespaceUri': 'नेमस्पेस URI',
  'detail.localName': 'लोकल नेम',
  'detail.classes': 'क्लासेज',
  'detail.noProperties': 'इस मानक में कोई गुण नहीं हैं।',
  'detail.noClasses': 'इस मानक में कोई क्लासेज़ नहीं हैं।',
  'vocabularies.add': 'मानक जोड़ें',
  'vocabularies.legend': 'मानक',
  'vocabularies.editLabel': 'इस मानक को संपादित करें',
  'vocabularies.deleteLabel': 'इस मानक को हटाएँ',
  'vocabularies.permanent':
    'Dublin Core और Dublin Core Type सॉफ्टवेयर का हिस्सा हैं, इसलिए इस मानक को संपादित या हटाया नहीं जा सकता।',
  'vocabularies.errorEmptyNamespace': 'नेमस्पेस URI खाली नहीं हो सकता।',
  'vocabularies.errorEmptyPrefix': 'प्रीफिक्स खाली नहीं हो सकता।',
  'vocabularies.errorPrefixPattern':
    'प्रीफिक्स में केवल अक्षर, संख्या, हाइफ़न और अंडरस्कोर हो सकते हैं।',
  'member.legend': 'सदस्य',
  'member.selectVocabulary': 'एक मानक चुनें',
  'member.errorEmptyLocalName': 'स्थानीय नाम खाली नहीं हो सकता।',
  'member.errorEmptyVocabulary': 'एक मानक निर्धारित किया जाना चाहिए।',
  'properties.add': 'प्रॉपर्टी जोड़ें',
  'properties.editLabel': 'इस प्रॉपर्टी को संपादित करें',
  'properties.deleteLabel': 'इस प्रॉपर्टी को हटाएँ',
  'resourceClasses.add': 'क्लास जोड़ें',
  'resourceClasses.editLabel': 'इस क्लास को संपादित करें',
  'resourceClasses.deleteLabel': 'इस क्लास को हटाएँ',
  'route.invalidVocabularyId': '"{param}" एक वैध मानक आईडी नहीं है।',
  'route.invalidPropertyId': '"{param}" एक वैध प्रॉपर्टी आईडी नहीं है।',
  'route.invalidClassId': '"{param}" एक वैध क्लास आईडी नहीं है।',
};
