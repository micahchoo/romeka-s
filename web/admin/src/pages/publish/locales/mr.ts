/**
 * The Marathi Publish panel messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Marathi. Correct
 * a string here; a later run keeps it. It is typed `PublishMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { PublishMessages } from '../messages';

export const mr: PublishMessages = {
  'publish.heading': 'प्रकाशित करा',
  'publish.ledeTermList':
    'प्रकाशन केल्यामुळे या सूचीला आणि त्यातील प्रत्येक संज्ञेला एक कायमस्वरूपी वेब पत्ता मिळतो, जो लोक आणि प्रोग्राम्स वाचू शकतात.',
  'publish.ledeVocabulary':
    'प्रकाशन केल्यामुळे या शब्दसंग्रहाला आणि त्यातील प्रत्येक संज्ञेला एक कायमस्वरूपी वेब पत्ता मिळतो, जो लोक आणि प्रोग्राम्स वाचू शकतात.',
  'publish.loading': 'प्रकाशन लोड होत आहे...',
  'publish.stateDraft': 'मसुदा. अद्याप प्रकाशित झालेला नाही.',
  'publish.statePublished': 'प्रकाशित. आवृत्ती {version}, {date}.',
  'publish.publicUrl': 'सार्वजनिक पत्ता',
  'publish.promise':
    'एकदा प्रकाशित झाले की, संज्ञा जोडल्या किंवा कालबाह्य केल्या जाऊ शकतात, परंतु त्या कधीही काढल्या किंवा पुन्हा नामांकित केल्या जाऊ शकत नाहीत आणि पत्ता कधीही बदलत नाही. इतर संग्रहण आधीच त्यांच्याशी जोडलेले असू शकतात.',
  'publish.slugLabel': 'पत्त्याचे नाव',
  'publish.slugHint': 'लोअरकेस अक्षरे, अंक आणि एक हायफन. याची {url} वर सेवा दिली जाते',
  'publish.slugInvalid': 'लोअरकेस अक्षरे, अंक आणि एक हायफन, जास्तीत जास्त 64 अक्षरे',
  'publish.namespaceLegend': 'संज्ञांचे पत्ते',
  'publish.namespaceArchive': 'या संग्रहाचा पत्ता: {url}',
  'publish.namespacePersistent': 'w3id.org किंवा purl.org वर कायमस्वरूपी पत्ता',
  'publish.namespaceWhy':
    'कायमस्वरूपी पत्ता दुसऱ्या डोमेनवर जाण्यानंतरही टिकतो: सेवा येथे पुनर्निर्देशित करते, त्यामुळे तुमच्या संज्ञांच्या लिंक्स चालू राहतात जर हा संग्रह हलला तर.',
  'publish.namespaceLabel': 'कायमस्वरूपी नेमस्पेस',
  'publish.namespaceHint':
    'उदाहरण https://w3id.org/your-archive/places/. हे / किंवा # ने समाप्त करा.',
  'publish.namespaceInvalid':
    'w3id.org किंवा purl.org वर असे एक पत्ता वापरा जो / किंवा # ने समाप्त होतो.',
  'publish.redirectHelp':
    'नंतर सेवा येथे पुनर्निर्देशित करण्यासाठी विनंती करा, उदाहरणार्थ या w3id.org नियमासह:',
  'publish.namespace': 'नेमस्पेस',
  'publish.termsHeading': 'संज्ञा',
  'publish.columnTerm': 'संज्ञा',
  'publish.columnUri': 'URI',
  'publish.columnState': 'स्थिती',
  'publish.columnChange': 'बदल',
  'publish.termDraft': 'अद्याप प्रकाशित झाले नाही',
  'publish.termPublished': 'प्रकाशित',
  'publish.termDeprecated': 'कालबाह्य',
  'publish.termDeprecationPending': 'पुढील आवृत्तीतून कालबाह्य',
  'publish.deprecate': 'कालबाह्य करणे',
  'publish.deprecateLabel': '{term} कालबाह्य करणे',
  'publish.undeprecate': 'ठेवणे',
  'publish.undeprecateLabel': '{term} चा वापर सुरू ठेवणे',
  'publish.links': 'दुवे',
  'publish.linksLabel': '{term} चे अधिकृत संस्थांशी असलेले दुवे',
  'publish.exactMatch': 'इतरत्र तोच शब्द (अचूक जुळणी)',
  'publish.closeMatch': 'इतरत्र जवळचा शब्द (जवळचा जुळणी)',
  'publish.linksHint': 'GeoNames किंवा Wikidata सारख्या अधिकृत संस्थेतील या संज्ञेचा पत्ता.',
  'publish.saveLinks': 'दुवे जतन करा',
  'publish.matchInvalid': 'http:// किंवा https:// ने सुरू होणारा पत्ता वापरा.',
  'publish.publish': 'प्रकाशित करा',
  'publish.publishVersion': 'आवृत्ती {version} प्रकाशित करा',
  'publish.noChanges': 'आवृत्ती {version} पासून काहीही बदललेले नाही.',
  'publish.pending':
    'आवृत्ती {version} पासून केलेले बदल अद्यात सार्वजनिक नाहीत. त्यांना प्रकाशित करण्यासाठी, प्रकाशित करा.',
  'publish.published': 'प्रकाशित आवृत्ती {version}.',
  'publish.versionsHeading': 'आवृत्त्या',
  'publish.versionLine': 'आवृत्ती {version}, {date}, {terms} अटी',
  'publish.previewHeading': 'टर्टलमध्ये पूर्वदृश्य',
  'publish.replacedBy': '“{term}” द्वारे बदलले',
};
