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
  'server.siteItemsOneSelector': 'नाव एकतर "query", किंवा "items", "item_sets" किंवा दोन्ही',
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
  'server.settingNotCount': '{key} ही शून्य पेक्षा मोठी पूर्ण संख्या किंवा null असणे आवश्यक आहे.',
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
  'server.importAdminsOnly': 'केवळ जागतिक किंवा साइट प्रशासकच आर्काइव्ह आयात करू शकतात.',
  'server.importNotFound': '{id} आयडीसह कोणतेही आयात अस्तित्वात नाही.',
  'server.importRequestUnreadable': 'विनंती वाचता येत नाही: {reason}',
  'server.importPlanRefused': 'आयातीच्या सेटिंग्ज जतन करता येत नाहीत: {reason}',
  'server.importStale':
    'तुम्ही हे आयात उघडल्यानंतर दुसऱ्या कोणीतरी त्यात बदल जतन केले आहेत. त्यांचे बदल पाहण्यासाठी ते पुन्हा लोड करा.',
  'server.importPlanFrozen':
    'ही आयात आधीच सुरू झाली आहे, त्यामुळे तिच्या सेटिंग्ज बदलता येणार नाहीत. त्या बदलण्यासाठी तिची प्रत तयार करा.',
  'server.importV1Plan':
    'ही आयात जुन्या आयातकाद्वारे केली गेली आहे, त्यामुळे ती येथे उघडता किंवा कॉपी करता येणार नाही.',
  'server.importTypeExists':
    'Omeka मध्ये "{label}" नावाचा आधीच एक आयटम प्रकार आहे. नवीन प्रकारासाठी दुसरे नाव निवडा.',
  'server.importDeleteNotDraft':
    'केवळ अद्याप न जोडलेली आयात हटवता येते. जी आयात चालली आहे, ती पूर्वीच्या आयातींमध्ये राहते.',
  'server.importChanged':
    'शेवटच्या तपासणीनंतर फाईल्स किंवा आयातीची सेटिंग्ज बदलली आहेत. ती पुन्हा चालवण्यापूर्वी आयातीची पुन्हा तपासणी करा.',
  'server.importBlocked':
    'जोपर्यंत तपासणी सूचीतील समस्या सुटत नाहीत, तोपर्यंत ही आयात चालू शकत नाही.',
  'server.importNothingToRun': 'या आयातीमध्ये लिहिण्यासारखे काहीही नाही.',
  'server.importConfirmDelete':
    'ही आयात {count} आयटम, मीडिया आणि आयटम सेट हटवते आणि ही क्रिया पूर्ववत करता येत नाही. ती चालवण्यासाठी त्या संख्येची पुष्टी करा.',
  'server.importBusy':
    'दुसरी आयात सुरू आहे किंवा पूर्ववत केली जात आहे. ती संपल्यावर पुन्हा प्रयत्न करा.',
  'server.importCannotRun':
    'केवळ अशी आयात जी अद्याप जोडलेली नाही, जी शेवट होण्यापूर्वी थांबली आहे किंवा जी अपयशासह पूर्ण झाली आहे, ती चालू शकते.',
  'server.importCannotUndo':
    'केवळ अशी आयात जी पूर्ण झाली आहे किंवा जी शेवट होण्यापूर्वी थांबली आहे, ती पूर्ववत केली जाऊ शकते.',
  'server.importNotRunning': 'ही आयात चालू नाही.',
  'server.importNoRoot': '"{root}" नावाचे कोणतेही आर्काइव्ह फोल्डर नाही.',
  'server.importOutsideRoot': '"{path}" हे फोल्डर आर्काइव्ह फोल्डरच्या बाहेर आहे.',
  'server.importFolderMissing': 'आर्काइव्हमध्ये "{path}" हे फोल्डर नाही.',
  'server.importNotAFolder': '"{path}" ही एक फाईल आहे, फोल्डर नाही.',
  'server.importScanBusy':
    'ही आयात चालू आहे किंवा पूर्ववत केली जात आहे. ती संपल्यावर पुन्हा फाईल्स तपासा.',
  'server.importNoLeftFile':
    'या आयातीमध्ये अद्याप वगळलेल्या फाईल्सची सूची नाही. जेव्हा एखादी प्रक्रिया पूर्ण होते, तेव्हा ती लिहिली जाते.',
  'server.importJobNotStarted': 'आयात सुरू होऊ शकली नाही: {reason}',
  'server.importListColumnUnknown':
    'सूचीमध्ये "{column}" हा स्तंभ असू शकत नाही. एखादा गुणधर्म, आयटम सेट्स, आयटम प्रकार किंवा दृश्यमानता निवडा.',
  'server.importListOneFile': 'फॉर्ममध्ये एक सूची फाईल पाठवा.',
  'server.importListType':
    '"{name}" ही .csv, .tsv, .xlsx, .xls किंवा .ods फाईल नाही. ही सूची यांपैकी एका स्वरूपात जतन करा आणि नंतर ती पुन्हा पाठवा.',
  'server.importListTooLarge': 'सूची 50 MB पेक्षा मोठी आहे.',
  'server.importListUnread': 'सूची वाचता येत नाही: {reason}',
  'server.importListNoIdentifier':
    'सूचीमध्ये ओळखकर्ता (Identifier) स्तंभ नाही, त्यामुळे त्याच्या ओळी त्यांचे आयटम शोधू शकत नाहीत. तुम्ही डाउनलोड केलेल्या सूचीचा Identifier स्तंभ कायम ठेवा.',
  'server.importPairsColumns':
    'सूचीमध्ये Row स्तंभ आणि Files स्तंभ असणे आवश्यक आहे. तुम्ही डाउनलोड केलेल्या सूचीतील दोन्ही स्तंभ तसेच ठेवा.',
  'server.importFileUnreadable': '"{path}" ही फाईल वाचता आली नाही: {reason}',
  'server.importFileRefused': '"{path}" ही फाईल जोडली गेली नाही: {reason}',
  'server.importFileNotStored': '"{path}" ही फाईल जतन करता आली नाही: {reason}',
  'server.importSourceRefused': '"{source}" जोडले गेले नाही: {reason}',
  'server.importMediaNoSource':
    'या मीडियामध्ये जोडण्यासाठी कोणतीही फाईल किंवा कोणताही पत्ता दिलेला नाही.',
  'server.importMediaNoItem':
    'या मीडियाचा आयटम जोडला गेला नाही, त्यामुळे मीडिया देखील जोडला गेला नाही.',
  'server.importResourceGone':
    'या नोंदीत बदल करणारा आयटम, मीडिया किंवा आयटम सेट आता Omeka मध्ये नाही.',
  'server.importRecordNotSaved': 'ही नोंद जतन करता आली नाही: {reason}',
  'server.importLinksNotSaved': 'या नोंदीचे दुवे जतन करता आले नाहीत: {reason}',
  'server.importRecordNotDeleted': 'हे हटवले गेले नाही: {reason}',
  'server.importDeleteNotUndone':
    'ज्या आयातीने आयटम, मीडिया किंवा आयटम सेट हटवले आहेत, ते पूर्ववत करता येत नाहीत.',
  'server.importSitesNotJoined':
    'हा आयटम जोडला गेला, परंतु तो त्याच्या साइट्समध्ये समाविष्ट होऊ शकला नाही: {reason}',
  'server.importUndoEdited': 'आयातीने हे लिहिल्यानंतर यात संपादन केले गेले.',
  'server.importUndoLinked':
    'या आयातीबाहेरील एखादा आयटम, मीडिया किंवा आयटम सेट याच्याशी जोडलेला आहे.',
  'server.importUndoMediaAdded': 'या आयातीच्या बाहेरून या आयटममध्ये मीडिया जोडला गेला.',
  'server.importUndoMembersAdded': 'या आयातीबाहेरील आयटम या आयटम सेटमध्ये ठेवले गेले.',
  'server.importUndoLaterImport': 'या आयातीनंतर आयात {import} ने यात बदल केला.',
  'server.importUndoInUse':
    'या आयातीबाहेरील आयटम, मीडिया, आयटम सेट किंवा आयटम प्रकार याचा वापर करतात.',
  'server.importUndoItemKept':
    'या मीडियाचा आयटम जतन केला जातो, त्यामुळे मीडिया देखील जतन केला जातो.',
  'server.importUndoMemberKept':
    'या आयटम सेटमधील एक आयटम जतन केला जातो, त्यामुळे आयटम सेट देखील जतन केला जातो.',
  'server.importUndoStopped': 'पूर्ववत करणे शेवटापूर्वीच थांबले: {reason}',
  'server.importUploadOwnerOnly':
    'केवळ या आयातीची सुरुवात करणाऱ्या व्यक्तीला किंवा जागतिक प्रशासकाला यात फाईल्स जोडता येतात.',
  'server.importUploadNotDraft':
    'फाईल्स केवळ तेव्हाच जोडल्या जाऊ शकतात जेव्हा आयात सुरू होण्यापूर्वी. अधिक फाईल्स जोडण्यासाठी, या आयातीची प्रत तयार करा.',
  'server.importUploadPath': '"{path}" हे फाईल नाव वापरता येणार नाही: {reason}.',
  'server.importUploadTooLarge':
    'ही फाईल {limit} bytes पेक्षा मोठी आहे, जी आयातीसाठी लागणारी सर्वात मोठी फाईल आहे.',
  'server.importUploadFull':
    'या आयातीच्या फाईल्स {limit} bytes पेक्षा जास्त होतील, जी एका आयातीमध्ये सामावणाऱ्या सर्वात जास्त फाईल्स आहेत. काही फाईल्स काढून टाका, किंवा त्यांच्या येण्याची वाट पहा, आणि पुन्हा प्रयत्न करा.',
  'server.importUploadChunkTooLarge':
    'या विनंतीमध्ये {limit} bytes पेक्षा जास्त डेटा आहे. फाईल लहान भागांत पाठवा.',
  'server.importUploadInProgress':
    '"{path}" अजूनही मार्गावर आहे. पुन्हा सुरू करण्याऐवजी त्या अपलोडसह पुढे जा.',
  'server.importUploadNotFound': 'या आयातीमध्ये "{upload}" आयडीसह कोणतीही अपलोड नाही.',
  'server.importUploadOffset':
    'सर्व्हरने या फाईलचे {received} bytes जतन केले आहेत. त्यांच्यानंतर येणारे bytes पाठवा.',
  'server.importUploadPastLength': 'हे bytes या अपलोडने घोषित केलेल्या आकारापेक्षा जास्त आहेत.',
  'server.importUploadLength': 'फाईलचा आकार bytes मध्ये कळवण्यासाठी Upload-Length पाठवा.',
  'server.importUploadMetadata': 'फाईलचे नाव "filename" म्हणून आणि base64 मध्ये त्याचे नाव पाठवा.',
  'server.importUploadOffsetMissing':
    'आधीच पाठवलेल्या bytes ची संख्या दर्शवण्यासाठी Upload-Offset पाठवा.',
  'server.importUploadContentType': 'Content-Type application/offset+octet-stream सह बाइट्स पाठवा.',
  'server.importUploadVersion':
    'हा सर्व्हर resumptions साठी version 1.0.0 प्रोटोकॉल बोलतो. Tus-Resumable: 1.0.0 पाठवा.',
  'server.importUploadInterrupted':
    'विनंती संपण्यापूर्वी bytes येणे थांबले, त्यामुळे त्यातील कोणीही जतन केले गेले नाही. Offset मागा आणि ते पुन्हा पाठवा.',
  'server.importZipEscapes':
    '"{name}" हा ZIP मधील फोल्डरच्या बाहेर पडला असता, म्हणून तो वगळला गेला.',
  'server.importZipLink': '"{name}" हा ZIP मधील एक link आहे, म्हणून तो वगळला गेला.',
  'server.importZipUnusable':
    '"{name}" हा ZIP मधील असा एक घटक आहे ज्याचे नाव वापरता येत नाही ({reason}), म्हणून तो वगळला गेला.',
  'server.importZipUnsupported':
    '"{name}" हा ZIP मधील असा एक घटक आहे जो Romeka S उलगडू शकत नाही, म्हणून तो वगळला गेला.',
  'server.importZipRenamed':
    '"{name}" हा ZIP मधील असा एक घटक आहे ज्याचे नाव आधीच्या फाईलसारखेच आहे (अक्षरांचा आकार दुर्लक्षित केल्यास), म्हणून तो "{renamed}" म्हणून जोडला गेला.',
  'server.importZipBomb':
    'ZIP नाकारला गेला: "{name}" हा 100 पेक्षा जास्त वेळा त्याच्या संकुचित आकारापेक्षा जास्त उलगडतो. त्यातील काहीही जोडले गेले नाही.',
  'server.importZipFull':
    'ZIP नाकारला गेला: त्याच्या फाईल्स {limit} bytes पेक्षा जास्त असतील, जे एका आयात प्रक्रियेत सर्वाधिक धरले जातात. त्यातील काहीही जोडले गेले नाही.',
  'server.importZipTooMany':
    'ZIP नाकारला गेला: यात {count} फाईल्स आहेत, 100,000 पेक्षा जास्त. त्यातील काहीही जोडले गेले नाही.',
  'server.importZipFileTooLarge':
    'ZIP नाकारला गेला: "{name}" हा {limit} bytes पेक्षा मोठा आहे, जी आयात प्रक्रियेत स्वीकारली जाणारी सर्वात मोठी फाईल आहे. त्यातील काहीही जोडले गेले नाही.',
  'server.importZipUnreadable':
    'ZIP नाकारला गेला: तो वाचता आला नाही ({reason}). त्यातील काहीही जोडले गेले नाही.',
  'server.importZipUnpacked': 'ZIP उलगडला गेला: {count} फाईल्स जोडल्या गेल्या.',
  'server.importSweepDone':
    '{imports} आयातीमधील फाईल्स काढून टाकल्या, आणि {uploads} अपूर्ण अपलोड्स.',
  'server.mediaUploadNotFound':
    'या खात्यात "{upload}" आयडीसह कोणतेही टप्प्याटप्प्याने केलेले अपलोड नाही.',
  'server.mediaUploadBusy':
    'हे अपलोड "{upload}" आता जोडले जात आहे. ते पूर्ण होण्याची प्रतीक्षा करा आणि पुन्हा प्रयत्न करा.',
  'server.mediaUploadIncomplete':
    'या अपलोडमध्ये त्याच्या {length} बाइट्सपैकी {received} बाइट्स आहेत. ते जोडण्यापूर्वी उर्वरित भाग पाठवा.',
  'server.mediaUploadIngester': 'टप्प्याटप्प्याने केलेले अपलोड अपलोड इनगेस्टरसह जोडले जाते.',
  'server.mediaUploadInline':
    'टप्प्याटप्प्याने केलेले अपलोड त्याच्या स्वतःच्या मीडिया क्रिएशनसह जोडले जाते, आयटमच्या आत नाही.',
  'server.uploadTooLarge':
    'ही फाईल {limit} बाइट्सपेक्षा मोठी आहे, जी मीडिया अपलोडद्वारे स्वीकारली जाणारी सर्वात मोठी फाईल आहे.',
  'server.mediaLinkInvalid': '"{url}" हा दुवा http किंवा https पत्ता नाही.',
  'server.mediaLinkUnreadable': '"{url}" हा दुवा वाचता आला नाही: {reason}',
  'server.mediaLinkStatus': '"{url}" या दुव्याने {status} {reason} दिले.',
  'server.uploadItem': 'अपलोड केलेल्या आयटमची आयडी अशा आयटमचे नाव असावी जी अस्तित्वात आहे.',
  'server.mediaUploadSweepDone':
    '{count} टप्प्याटप्प्याने केलेले अपलोड्स त्यांच्या ठेवलेल्या वेळेनुसार काढले गेले.',
};
