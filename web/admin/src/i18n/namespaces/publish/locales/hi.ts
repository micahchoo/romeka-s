/**
 * The Hindi Publish panel messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Hindi. Correct
 * a string here; a later run keeps it. It is typed `PublishMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { PublishMessages } from '../index';

export const hi: PublishMessages = {
  'publish.heading': 'प्रकाशित करें',
  'publish.ledeTermList':
    'प्रकाशन इस सूची और इसके प्रत्येक शब्द को एक स्थायी वेब पता देता है जिसे लोग और प्रोग्राम पढ़ सकते हैं।',
  'publish.ledeVocabulary':
    'प्रकाशन इस मानक और इसके प्रत्येक शब्द को एक स्थायी वेब पता देता है जिसे लोग और प्रोग्राम पढ़ सकते हैं।',
  'publish.loading': 'प्रकाशन लोड हो रहा है...',
  'publish.stateDraft': 'मसौदा। अभी तक प्रकाशित नहीं हुआ।',
  'publish.statePublished': 'प्रकाशित। संस्करण {version}, {date}।',
  'publish.publicUrl': 'सार्वजनिक पता',
  'publish.promise':
    'एक बार प्रकाशित होने के बाद, शब्दों को जोड़ा या अप्रचलित किया जा सकता है, लेकिन उन्हें कभी हटाया या पुन: नामित नहीं किया जा सकता है, और पता कभी नहीं बदलता है। अन्य संग्रह पहले से ही उनसे जुड़ सकते हैं।',
  'publish.slugLabel': 'पता का नाम',
  'publish.slugHint': 'छोटे अक्षर, अंक और एकल हाइफ़न। इसकी सेवा {url} पर की जाती है',
  'publish.slugInvalid': 'छोटे अक्षरों, अंकों और एकल हाइफ़न का उपयोग करें, अधिकतम 64 वर्ण।',
  'publish.namespaceLegend': 'शब्दों के पते',
  'publish.namespaceArchive': 'इस संग्रह का पता: {url}',
  'publish.namespacePersistent': 'w3id.org या purl.org पर एक स्थायी पता',
  'publish.namespaceWhy':
    'एक स्थायी पता किसी अन्य डोमेन पर स्थानांतरण के बाद भी बना रहता है: सेवा इसे यहाँ पुनर्निर्देशित करती है, इसलिए आपके शब्दों के लिंक काम करते रहते हैं यदि यह संग्रह स्थानांतरित हो जाता है।',
  'publish.namespaceLabel': 'स्थायी नेमस्पेस',
  'publish.namespaceHint':
    'उदाहरण https://w3id.org/your-archive/places/. इसे / या # के साथ समाप्त करें।',
  'publish.namespaceInvalid':
    'w3id.org या purl.org पर एक पते का उपयोग करें जो / या # के साथ समाप्त होता है।',
  'publish.redirectHelp':
    'फिर सेवा को यहाँ पुनर्निर्देशित करने के लिए कहें, उदाहरण के लिए इस w3id.org नियम के साथ:',
  'publish.namespace': 'नेमस्पेस',
  'publish.termsHeading': 'शब्द',
  'publish.columnTerm': 'शब्द',
  'publish.columnUri': 'URI',
  'publish.columnState': 'स्थिति',
  'publish.columnChange': 'परिवर्तन',
  'publish.termDraft': 'अभी तक प्रकाशित नहीं',
  'publish.termPublished': 'प्रकाशित',
  'publish.termDeprecated': 'अप्रचलित',
  'publish.termDeprecationPending': 'अगले संस्करण से अप्रचलित',
  'publish.deprecate': 'अप्रचलित करना',
  'publish.deprecateLabel': '{term} को अप्रचलित करना',
  'publish.undeprecate': 'रखना',
  'publish.undeprecateLabel': '{term} को उपयोग में रखना',
  'publish.links': 'लिंक',
  'publish.linksLabel': '{term} के लिंक किसी प्राधिकारी से',
  'publish.exactMatch': 'वही शब्द कहीं और (exactMatch)',
  'publish.closeMatch': 'एक निकटवर्ती शब्द कहीं और (closeMatch)',
  'publish.linksHint': 'इस शब्द का पता GeoNames या Wikidata जैसे प्राधिकारी में।',
  'publish.saveLinks': 'लिंक सहेजना',
  'publish.matchInvalid': 'http:// या https:// से शुरू होने वाले पते का उपयोग करें।',
  'publish.publish': 'प्रकाशित करना',
  'publish.publishVersion': 'संस्करण {version} प्रकाशित करना',
  'publish.noChanges': 'संस्करण {version} के बाद से कुछ भी नहीं बदला है।',
  'publish.pending':
    'संस्करण {version} से परिवर्तन अभी तक सार्वजनिक नहीं हैं। उन्हें जोड़ने के लिए प्रकाशित करें।',
  'publish.published': 'संस्करण {version} प्रकाशित किया गया।',
  'publish.versionsHeading': 'संस्करण',
  'publish.versionLine': 'संस्करण {version}, {date}, {terms} शर्तें',
  'publish.previewHeading': 'टर्टल में पूर्वावलोकन',
  'publish.replacedBy': '“{term}” द्वारा प्रतिस्थापित',
};
