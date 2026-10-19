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
    'साझा शब्दावलियाँ जिनमें आपके फ़ील्ड और आइटम प्रकार सहेजे जाते हैं। आपको संग्रह को सूचीबद्ध करने के लिए उन्हें बदलने की आवश्यकता नहीं है।',
  'standards.define': '{term} का अर्थ',
  'standards.import': 'शब्दावली आयात करें',
  'import.heading': 'शब्दावली आयात करें',
  'import.lede':
    'RDF फ़ाइल या वेब पते से प्रकाशित शब्दावली लाएँ। इसकी क्लासेज और प्रॉपर्टीज़ आपके फ़ील्ड्स और आइटम टाइप्स के लिए उपलब्ध हो जाती हैं।',
  'import.submit': 'शब्दावली आयात करें',
  'import.working': 'शब्दावली पढ़ी जा रही है…',
  'source.legend': 'शब्दावली कहाँ से आती है',
  'source.upload': 'इस कंप्यूटर पर एक फ़ाइल',
  'source.url': 'एक वेब पता',
  'source.file': 'शब्दावली फ़ाइल',
  'source.urlLabel': 'फ़ाइल का वेब पता',
  'source.format': 'फ़ाइल प्रारूप',
  'source.formatHelp':
    'जब तक आयात यह न कहे कि यह प्रारूप नहीं बता सकता, तब तक इसे “स्वचालित रूप से पता लगाएँ” पर छोड़ दें।',
  'format.guess': 'स्वचालित रूप से पता लगाएँ',
  'format.rdfxml': 'RDF/XML (.rdf, .owl)',
  'format.turtle': 'टर्टल या नोटेशन3 (.ttl, .n3)',
  'format.ntriples': 'N-ट्रिपल्स (.nt)',
  'format.jsonld': 'JSON-LD (.jsonld)',
  'about.legend': 'शब्दावली के बारे में',
  'about.namespace': 'नेमस्पेस URI',
  'about.namespaceHelp':
    'शब्दावली के प्रत्येक क्लास और प्रॉपर्टी का वेब पता, जैसे http://xmlns.com/foaf/0.1/. केवल इसके अंतर्गत के शब्द आयात किए जाते हैं। यह आमतौर पर / या # पर समाप्त होता है।',
  'about.prefix': 'प्रीफ़िक्स',
  'about.prefixHelp':
    'प्रत्येक शब्द से पहले लिखी गई एक छोटी पहचान, जैसे foaf:name। अक्षरों, संख्याओं, हाइफ़न और अंडरस्कोर का उपयोग करें।',
  'about.label': 'लेबल',
  'about.labelHelp': 'इस शब्दावली के लिए लोगों द्वारा देखा जाने वाला नाम।',
  'about.comment': 'विवरण (वैकल्पिक)',
  'vocabularies.advancedSummary': 'उन्नत',
  'advanced.lang': 'पसंदीदा भाषा',
  'advanced.langHelp':
    'en या fr जैसी भाषा टैग। उस भाषा में लेबल और टिप्पणियाँ रखी जाती हैं। इसके बिना, या उस शब्द के लिए जिसमें यह नहीं है, फ़ाइल में पहला शब्द रखा जाता है।',
  'advanced.label': 'लेबल प्रॉपर्टी',
  'advanced.labelHelp':
    'असामान्य प्रॉपर्टी के साथ शब्दावली का नामकरण करने वाली शब्दावली के लिए। कोण कोष्ठक में पूर्ण प्रॉपर्टी URI लिखें, जैसे <http://schema.org/name>। इसके बिना, skos:prefLabel, rdfs:label, foaf:name, rss:title और dc:title को क्रमवार आज़माया जाता है।',
  'advanced.comment': 'टिप्पणी प्रॉपर्टी',
  'advanced.commentHelp':
    'प्रत्येक शब्द के विवरण के लिए, समान। इसके बिना, rdfs:comment पढ़ा जाता है।',
  'error.noFile': 'आयात करने के लिए एक फ़ाइल चुनें।',
  'error.noUrl': 'फ़ाइल का वेब पता दर्ज करें।',
  'update.toggle': 'फ़ाइल या वेब पते से अपडेट करें',
  'update.lede':
    'इस शब्दावली की तुलना इसकी फ़ाइल की नई प्रति से करें। आप कुछ भी सहेजे जाने से पहले हर बदलाव देखते हैं।',
  'update.compare': 'तुलना',
  'update.comparing': 'तुलना की जा रही है…',
  'diff.review': 'इन परिवर्तनों की समीक्षा करें, फिर उन्हें लागू करें।',
  'diff.none': 'यह शब्दावली पहले से ही फ़ाइल से मेल खाती है। बदलने के लिए कुछ भी नहीं है।',
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
};
