/**
 * The Marathi Standards and Fields page messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Marathi. Correct
 * a string here; a later run keeps it. It is typed `StandardsMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { StandardsMessages } from '../index';

export const mr: StandardsMessages = {
  'standards.lede':
    'तुमच्या क्षेत्रांसाठी आणि आयटम प्रकारांसाठी सामायिक शब्दसंग्रह. तुम्हाला संग्रह सूचीबद्ध करण्यासाठी ते बदलण्याची आवश्यकता नाही.',
  'standards.define': '{term} चा अर्थ काय',
  'standards.import': 'शब्दसंग्रह आयात करा',
  'import.heading': 'शब्दसंग्रह आयात करा',
  'import.lede':
    'आरडीएफ फाईल किंवा वेब पत्त्यावरून प्रकाशित शब्दसंग्रह आणा. त्याचे वर्ग आणि गुणधर्म तुमच्या फील्ड्स आणि आयटम प्रकारांना उपलब्ध होतात.',
  'import.submit': 'शब्दसंग्रह आयात करा',
  'import.working': 'शब्दसंग्रह वाचत आहे…',
  'source.legend': 'शब्दसंग्रह कुठून येतो',
  'source.upload': 'या संगणकावरील फाईल',
  'source.url': 'वेब पत्ता',
  'source.file': 'शब्दसंग्रह फाईल',
  'source.urlLabel': 'फाईलचा वेब पत्ता',
  'source.format': 'फाईल स्वरूप',
  'source.formatHelp':
    'जोपर्यंत आयात फॅक्ट सांगत नाही की ते स्वरूप सांगू शकत नाही, तोपर्यंत हे “स्वयंचलितपणे शोध” वर ठेवा.',
  'format.guess': 'स्वयंचलितपणे शोध',
  'format.rdfxml': 'आरडीएफ/एक्सएमएल (.rdf, .owl)',
  'format.turtle': 'टर्टल किंवा नोटेशन3 (.ttl, .n3)',
  'format.ntriples': 'एन-ट्रिपल्स (.nt)',
  'format.jsonld': 'JSON-LD (.jsonld)',
  'about.legend': 'शब्दसंग्रह बद्दल',
  'about.namespace': 'नेमस्पेस यूआरआय',
  'about.namespaceHelp':
    'शब्दसंग्रहातील प्रत्येक वर्ग आणि गुणधर्माचा वेब पत्ता, जसे की http://xmlns.com/foaf/0.1/. फक्त त्याखालील संज्ञा आयात केल्या जातात. हे सहसा / किंवा # ने संपते.',
  'about.prefix': 'उपसर्ग',
  'about.prefixHelp':
    'प्रत्येक संज्ञेच्या आधी लिहिलेले एक लहान नाव, जसे की foaf:name. अक्षरे, अंक, हायफन आणि अंडरस्कोर वापरा.',
  'about.label': 'लेबल',
  'about.labelHelp': 'या शब्दसंग्रहासाठी लोकांना दिसणारे नाव.',
  'about.comment': 'वर्णन (पर्यायी)',
  'vocabularies.advancedSummary': 'प्रगत',
  'advanced.lang': 'पसंतीची भाषा',
  'advanced.langHelp':
    'en किंवा fr सारखी भाषा टॅग. त्या भाषेतील लेबल्स आणि टिप्पण्या जतन केल्या जातात. एक नसल्यास, किंवा ज्या संज्ञेमध्ये त्यापैकी काहीही नाही, त्यातील पहिली संज्ञा फाईलमध्ये जतन केली जाते.',
  'advanced.label': 'लेबल गुणधर्म',
  'advanced.labelHelp':
    'असामान्य गुणधर्मासह शब्दसंग्रहाचे नाव देणाऱ्या शब्दसंग्रहासाठी. कोनांमधील पूर्ण गुणधर्म URI लिहा, जसे की <http://schema.org/name>. एक नसल्यास, skos:prefLabel, rdfs:label, foaf:name, rss:title आणि dc:title एकामागून एक वापरून पाहिले जातात.',
  'advanced.comment': 'टिप्पणी गुणधर्म',
  'advanced.commentHelp': 'प्रत्येक संज्ञेच्या वर्णनासाठी, rdfs:comment वाचले जाते.',
  'error.noFile': 'आयात करण्यासाठी एक फाईल निवडा.',
  'error.noUrl': 'फाईलचा वेब पत्ता प्रविष्ट करा.',
  'update.toggle': 'फाईल किंवा वेब पत्त्यावरून अद्ययावत करा',
  'update.lede':
    'या शब्दसंग्रहाची त्याच्या फाईलच्या नवीन प्रतीशी तुलना करा. तुम्ही काहीही जतन करण्यापूर्वी प्रत्येक बदल पाहू शकता.',
  'update.compare': 'तुलना करा',
  'update.comparing': 'तुलना करत आहे…',
  'diff.review': 'तुम्ही हे बदल लागू करण्यापूर्वी त्यांचे पुनरावलोकन करा.',
  'diff.none': 'हा शब्दसंग्रह आधीच फाईलशी जुळतो. बदलण्यासाठी काहीही नाही.',
  'diff.newClasses': 'नवीन वर्ग ({count})',
  'diff.newProperties': 'नवीन गुणधर्म ({count})',
  'diff.changed': 'बदललेले लेबल्स आणि टिप्पण्या ({count})',
  'diff.removed': 'फाईलमध्ये आता अस्तित्वात नाही ({count})',
  'diff.removedNote':
    'हे संग्रहात राहतात, तसेच त्यांना वापरणाऱ्या व्हॅल्यूज आणि आयटम प्रकार देखील राहतात. जर तुम्हाला आता त्यांची गरज नसेल, तर स्वतःच्या पृष्ठावरून एक हटवा.',
  'diff.term': 'संज्ञा',
  'diff.label': 'लेबल',
  'diff.comment': 'टिप्पणी',
  'diff.what': 'काय बदलले',
  'diff.now': 'आता',
  'diff.file': 'फाईलमध्ये',
  'diff.blank': '(काहीही नाही)',
  'diff.apply': 'या बदलांचा वापर करा',
  'diff.applying': 'लागू करत आहे…',
  'diff.cancel': 'रद्द करा',
  'diff.applied': 'बदल जतन केले आहेत.',
};
