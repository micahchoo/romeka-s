/**
 * The Marathi server messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Marathi. Correct
 * a string here; a later run keeps it. It is typed `ServerCatalogue`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { ServerCatalogue } from '../serverMessages';

export const mrServer: ServerCatalogue = {
  'server.methodNotAllowed': 'पद्धत अनुमत नाही',
  'server.notFound': 'सापडले नाही',
  'server.badRequest': 'चुकीची विनंती',
  'server.requestTimeout': 'विनंतीची वेळ संपली',
  'server.bodyTooLarge': 'विनंतीचा आकार खूप मोठा',
  'server.internalError': 'अंतर्गत सर्व्हर त्रुटी',
  'server.tooManyRequests': 'खूप जास्त विनंत्या',
  'server.unknownResource': 'API "{resource}" संसाधनाला समर्थन देत नाही.',
  'server.adapterNotImplemented': '{resource} अडॅप्टर {operation} ही प्रक्रिया राबवत नाही.',
  'server.endpointNotImplemented': '{endpoint} एंडपॉईंट अद्याप कार्यान्वित केलेला नाही.',
  'server.permissionDenied':
    'सध्याच्या वापरकर्त्याला {operation} {resource} संसाधनासाठी परवानगी नाकारली आहे.',
  'server.entityNotFound': '{criteria} निकषांसह {class} घटक सापडला नाही',
  'server.jsonSyntax': 'JSON: वाक्यरचना त्रुटी',
  'server.jsonNotContainer': 'JSON: मजकूर ऑब्जेक्ट किंवा ॲरे असणे आवश्यक आहे.',
  'server.invalidContentType':
    'अवैध Content-Type हेडर. "application/json" अपेक्षित आहे, {received} मिळाले.',
  'server.bodyNotObject': 'विनंतीचा मुख्य भाग (body) JSON ऑब्जेक्ट असणे आवश्यक आहे.',
  'server.siteItemsOneSelector': '"query", "items" किंवा "item_sets" पैकी नेमके एक नाव',
  'server.invalidCredentials': 'ईमेल किंवा पासवर्ड अवैध आहे',
  'server.inactiveAccount': 'वापरकर्ता सक्रिय नाही.',
  'server.unknownKey': 'अवैध पासवर्ड निर्मिती की.',
  'server.authenticationRequired': 'प्रमाणीकरण आवश्यक.',
  'server.invalidPassword': 'पासवर्ड अवैध आहे',
  'server.crossOriginRefused':
    'सत्र-प्रमाणित लिहिण्यासाठी (write) कॉन्फिगर केलेल्या बेस URL मधून येणे आवश्यक आहे.',
  'server.propertyNotRecognized': '"{term}" ही संज्ञा ओळखली गेली नाही.',
  'server.resourceNotFound': '{id} आयडीसह संसाधन सापडले नाही.',
  'server.templateRequiresValue':
    '"{template}" संसाधन टेम्पलेटला "{property}" मूल्याची आवश्यकता आहे',
  'server.itemSetsNotArray': 'आयटम सेट हे एक ॲरे (array) असणे आवश्यक आहे',
  'server.mediaNotArray': 'मीडिया हे एक ॲरे असणे आवश्यक आहे',
  'server.mediaNotThisItem': 'मीडिया या आयटमचा भाग असणे आवश्यक आहे.',
  'server.primaryMediaNotThisItem': 'प्राथमिक मीडिया या आयटमचा भाग असणे आवश्यक आहे.',
  'server.mediaIngesterRequired': 'मीडियाने इनगेस्टर सेट करणे आवश्यक आहे.',
  'server.mediaIngesterInvalid': 'मीडियाने वैध इनगेस्टर सेट करणे आवश्यक आहे.',
  'server.mediaItemRequired': 'मीडिया एखाद्या आयटमचा भाग असणे आवश्यक आहे.',
  'server.jobHeldByImport':
    'हे कार्य चालू असलेल्या किंवा पूर्ववत होणाऱ्या संग्रहाच्या आयातीचे आहे. आयात पूर्ण झाल्यावर तो हटवा.',
  'server.pageSavedSinceOpened': 'तुम्ही हे पृष्ठ उघडल्यानंतर कोणीतरी ते जतन केले.',
  'server.noFileUploaded': 'कोणतीही फाईल अपलोड केली गेली नाही',
  'server.assetMediaType':
    '"{file}" चे प्रमाणीकरण करताना त्रुटी. मीडिया प्रकार "{media}" सह फाईल्स साठवता येत नाहीत.',
  'server.assetExtension':
    '"{file}" चे प्रमाणीकरण करताना त्रुटी. रिझॉल्व्ह केलेल्या एक्सटेन्शन "{extension}" सह फाईल्स साठवता येत नाहीत.',
  'server.assetNameRequired': 'ॲसेटला नाव असणे आवश्यक आहे.',
  'server.classesNotArray': 'क्लासेस ॲरे असणे आवश्यक आहे',
  'server.propertiesNotArray': 'प्रॉपर्टीज ॲरे असणे आवश्यक आहे',
  'server.namespaceUriEmpty': 'नेमस्पेस URI रिकामे असू शकत नाही.',
  'server.namespaceUriTaken': 'नेमस्पेस URI "{uri}" आधीच वापरले गेले आहे.',
  'server.prefixEmpty': 'प्रीफिक्स रिकामे असू शकत नाही.',
  'server.prefixInvalid': 'प्रीफिक्समध्ये केवळ अक्षरे, संख्या, हायफन आणि अंडरस्कोअर असू शकतात.',
  'server.prefixTaken': 'प्रीफिक्स "{prefix}" आधीच वापरले गेले आहे.',
  'server.prefixReserved': 'प्रीफिक्स "{prefix}" आरक्षित आहे.',
  'server.labelEmpty': 'लेबल रिकामे असू शकत नाही.',
  'server.labelTaken': 'लेबल आधीच वापरले गेले आहे.',
  'server.localNameEmpty': 'स्थानिक नाव रिकामे असू शकत नाही.',
  'server.localNameTaken': 'स्थानिक नाव "{name}" आधीच वापरले गेले आहे.',
  'server.vocabularyRequired': 'शब्दसंग्रह सेट करणे आवश्यक आहे.',
  'server.duplicateTemplateProperty': 'आयडी {id} सह डुप्लिकेट प्रॉपर्टी जोडण्याचा प्रयत्न',
  'server.vocabularyNoFile': 'फाईल इम्पोर्ट धोरणासाठी कोणतीही फाईल निर्दिष्ट केलेली नाही.',
  'server.vocabularyNoUrl': 'URL इम्पोर्ट धोरणासाठी URL निर्दिष्ट केलेले नाही.',
  'server.vocabularyNotParsed': 'शब्दसंग्रह फाईलचे विश्लेषण करण्यात अपयश आले.',
  'server.vocabularyNotLoaded': 'URL वरून शब्दसंग्रह लोड करण्यात अपयश आले.',
  'server.vocabularyNoMembers':
    'कोणतेही क्लासेस किंवा गुणधर्म आढळले नाहीत. तुम्ही योग्य नेमस्पेस URI वापरला आहे का?',
  'server.vocabularyPermanent': 'कायमस्वरूपी शब्दसंग्रह संपादित करू शकत नाही',
  'server.vocabularyFormatUnknown': 'फाईलचे स्वरूप ओळखता आले नाही. सूचीमधून स्वरूप निवडा.',
  'server.vocabularyFormatInvalid': '"{format}" ही शब्दसंग्रह फाईल फॉरमॅट नाही.',
  'server.vocabularyUrlScheme': 'केवळ http आणि https URL आयात केले जाऊ शकतात.',
  'server.vocabularyUrlPrivate':
    'URL एक खाजगी किंवा स्थानिक नेटवर्क पत्ता दर्शवते, ज्यावरून सर्व्हर माहिती घेत नाही.',
  'server.vocabularyUrlTooLarge': 'URL वरील फाईल {size} पेक्षा मोठी आहे.',
  'server.vocabularyUrlTimeout': 'URL वरील सर्व्हरने वेळेत उत्तर दिले नाही.',
  'server.vocabularyUrlStatus': 'URL वरील सर्व्हरने {status} स्थितीसह उत्तर दिले.',
  'server.vocabularyChangeStale':
    '"{name}" ची तुलना केल्यापासून त्यात बदल झाला आहे. फाईलची पुन्हा तुलना करा.',
  'server.vocabularyMemberMissing': '"{name}" या शब्दसंग्रहाचा सदस्य नाही.',
  'server.nameEmpty': 'नाव रिकामे असू शकत नाही.',
  'server.emailTaken': 'ईमेल {email} आधीच वापरले गेले आहे.',
  'server.userRoleRequired': 'वापरकर्त्यांना भूमिका असणे आवश्यक आहे.',
  'server.invalidEmail': 'इनपुट ही वैध ईमेल पत्ता नाही. मूलभूत स्वरूप local-part@hostname वापरा',
  'server.siteTitleRequired': 'साइटला शीर्षक असणे आवश्यक आहे.',
  'server.siteThemeRequired': 'साइटला थीम असणे आवश्यक आहे.',
  'server.siteItemPoolRequired': 'साइटला आयटम पूल डेटा असणे आवश्यक आहे.',
  'server.homepageNotInSite': 'होमपेज त्याच्या मूळ साइटचा भाग असणे आवश्यक आहे.',
  'server.slugEmpty': 'स्लग रिकामे असू शकत नाही.',
  'server.slugInvalid': 'स्लगमध्ये फक्त अक्षरे, संख्या, अंडरस्कोर्स आणि हायफन्स असू शकतात.',
  'server.slugTaken': 'स्लग "{slug}" आधीच वापरला गेला आहे.',
  'server.pageSiteRequired': 'पेज साइटचा भाग असणे आवश्यक आहे.',
  'server.pageTitleRequired': 'पेजला शीर्षक असणे आवश्यक आहे.',
  'server.blockLayoutRequired': 'सर्व ब्लॉक्सना लेआउट असणे आवश्यक आहे.',
  'server.blockDataScalar': 'ब्लॉक डेटा स्केलर व्हॅल्यू नसावा.',
  'server.siteSettingsNotObject': 'अपेक्षित ऑब्जेक्ट किंवा नल, {kind} मिळाले.',
  'server.cssTooLarge':
    'कस्टमाइज्ड CSS: ओळ {line}, स्तंभ {column}: कस्टमाइज्ड CSS जास्तीत जास्त {max} बाइट्स असू शकते; हे {bytes} आहे.',
  'server.cssImport': 'कस्टमाइज्ड CSS: ओळ {line}, स्तंभ {column}: @import ला परवानगी नाही.',
  'server.cssDocument': 'कस्टमाइजित CSS: ओळ {line}, स्तंभ {column}: @document ला परवानगी नाही.',
  'server.cssForeignUrl':
    'कस्टमाइज्ड CSS: ओळ {line}, स्तंभ {column}: url({url}) ला परवानगी नाही; URL हे /files/ किंवा /assets/ अंतर्गत मार्ग असणे आवश्यक आहे.',
  'server.cssExpression':
    'कस्टमाइज्ड CSS: ओळ {line}, स्तंभ {column}: expression() ला परवानगी नाही.',
  'server.cssBehavior': 'कस्टमाइज्ड CSS: ओळ {line}, स्तंभ {column}: behavior ला परवानगी नाही.',
  'server.cssMozBinding':
    'कस्टमाइज्ड CSS: ओळ {line}, स्तंभ {column}: -moz-binding ला परवानगी नाही.',
  'server.cssStyleBreakout': 'कस्टमाइज्ड CSS: ओळ {line}, स्तंभ {column}: </style ला परवानगी नाही.',
  'server.cssBadUrl': 'कस्टमाइज्ड CSS: ओळ {line}, स्तंभ {column}: हे url( वैध CSS नाही.',
  'server.cssUnclosedString':
    'कस्टमाइज्ड CSS: ओळ {line}, स्तंभ {column}: ही स्ट्रिंग बंद केलेली नाही.',
  'server.cssStrayClose': 'कस्टमाइज्ड CSS: ओळ {line}, स्तंभ {column}: {bracket} कशाशीही जुळत नाही.',
  'server.navNotArray': 'अवैध नेव्हिगेशन: नेव्हिगेशन ॲरे असणे आवश्यक आहे',
  'server.navLinkMissingType': 'अवैध नेव्हिगेशन: लिंकचा प्रकार गहाळ आहे',
  'server.navLinkMissingData': 'अवैध नेव्हिगेशन: लिंकचा डेटा गहाळ आहे',
  'server.navInvalidLinkData': 'अवैध नेव्हिगेशन: अवैध लिंक डेटा',
  'server.navPageLinksNotUnique': 'अवैध नेव्हिगेशन: पेज लिंक्स अद्वितीय असणे आवश्यक आहे',
  'server.navLinksNotArray': 'अवैध नेव्हिगेशन: लिंक्स ॲरे असणे आवश्यक आहे',
  'server.navPageLinkMissingPage': 'अवैध नेव्हिगेशन: पेज लिंकमध्ये पेज आयडी गहाळ आहे',
  'server.navUrlMissingLabel': 'अवैध नेव्हिगेशन: URL लिंकमध्ये लेबल गहाळ आहे',
  'server.navUrlMissingUrl': 'अवैध नेव्हिगेशन: URL लिंकमध्ये URL गहाळ आहे',
  'server.navUrlInvalidCharacter': 'अवैध नेव्हिगेशन: URL लिंकमध्ये अवैध वर्ण आहे',
  'server.navUrlInvalidScheme': 'अवैध नेव्हिगेशन: URL लिंकमध्ये अवैध योजना आहे',
  'server.settingsDenied':
    'सध्याच्या वापरकर्त्याला जागतिक सेटिंग्जमध्ये प्रवेश करण्यासाठी परवानगी नाकारली गेली आहे.',
  'server.settingsNotObject': 'अपेक्षित एक ऑब्जेक्ट आहे.',
  'server.settingUnavailable': '{key} हे सेटिंग उपलब्ध नाही.',
  'server.settingNotText': '{key} हे सेटिंग स्ट्रिंग किंवा नल असणे आवश्यक आहे.',
  'server.settingNotFlag': '{key} हे सेटिंग सत्य, असत्य किंवा नल असणे आवश्यक आहे.',
  'server.settingNotNames': '{key} हे सेटिंग स्ट्रिंग्सची सूची किंवा नल असणे आवश्यक आहे.',
  'server.publicationTermsRemoved':
    'प्रकाशित शब्द काढता येत नाही किंवा त्याचे नाव बदलता येत नाही: {terms}. त्याऐवजी ते अप्रचलित (deprecate) करा.',
  'server.publicationNamespaceFixed':
    'प्रकाशित शब्दसंग्रह त्याचे नेमस्पेस राखतो, त्यामुळे त्याच्या संज्ञा त्यांचे URIs राखतात.',
  'server.publicationNotDeletable':
    'प्रकाशित संज्ञा सूची किंवा शब्दसंग्रह हटवता येत नाही. त्याऐवजी त्याच्या संज्ञा अप्रचलित करा.',
  'server.publicationDeprecationFixed':
    '"{term}" चे अप्रचलन प्रकाशित झाले आहे आणि ते मागे घेता येणार नाही.',
  'server.publicationItemsList':
    'वस्तूंच्या सूचीचे प्रकाशन शक्य नाही. प्रत्येक वस्तूचा स्वतःचा पत्ता आधीच आहे.',
  'server.publicationForeignNamespace':
    '{namespace} हे या संग्रहाचे नेमस्पेस नाही. /vocab/ पत्त्याखाली या संग्रहाच्या नेमस्पेस अंतर्गत शब्दसंग्रह प्रकाशित करा, किंवा w3id.org किंवा purl.org वर.',
  'server.publicationSlugInvalid':
    'पत्त्याच्या नावामध्ये लहान अक्षरे, अंक आणि एक हायफन, जास्तीत जास्त 64 अक्षरे वापरली जातात.',
  'server.publicationSlugTaken': '/vocab/{slug}/ हा पत्ता आधीच वापरला गेला आहे.',
  'server.publicationNamespaceNotPersistent':
    'या संग्रहाच्या नेमस्पेस व्यतिरिक्त इतर नेमस्पेस w3id.org किंवा purl.org वर असावा आणि / किंवा # ने समाप्त व्हावा.',
  'server.publicationAddressFixed':
    'प्रकाशित संज्ञा सूची किंवा शब्दसंग्रह तो पत्ता राखतो ज्यावर तो प्रथम प्रकाशित झाला होता.',
  'server.publicationTermNotPublished':
    '"{term}" अद्याप प्रकाशित झाले नसल्यामुळे, ते अप्रचलित करता येणार नाही. त्याऐवजी ते काढून टाका.',
  'server.publicationUnknownTerm': '"{term}" या सूचीचा किंवा शब्दसंग्रहाचा शब्द नाही.',
  'server.publicationMatchInvalid': 'जुळणी ही एक निरपेक्ष http किंवा https URI असणे आवश्यक आहे.',
  'server.publicationMatchTermLists': 'केवळ संज्ञा सूचीतील संज्ञा अधिकृततेशी जोडल्या जातात.',
  'server.publicationTermIriInvalid': '{uri} हा शब्द वैध IRI नाही.',
};
