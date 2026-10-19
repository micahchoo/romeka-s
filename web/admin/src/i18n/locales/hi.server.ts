/**
 * The Hindi server messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Hindi. Correct
 * a string here; a later run keeps it. It is typed `ServerCatalogue`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { ServerCatalogue } from '../serverMessages';

export const hiServer: ServerCatalogue = {
  'server.methodNotAllowed': 'विधि की अनुमति नहीं है',
  'server.notFound': 'नहीं मिला',
  'server.badRequest': 'खराब अनुरोध',
  'server.requestTimeout': 'अनुरोध का समय समाप्त',
  'server.bodyTooLarge': 'अनुरोध का शरीर बहुत बड़ा है',
  'server.internalError': 'आंतरिक सर्वर त्रुटि',
  'server.tooManyRequests': 'बहुत अधिक अनुरोध',
  'server.unknownResource': 'API "{resource}" संसाधन का समर्थन नहीं करता है।',
  'server.adapterNotImplemented': '{resource} एडेप्टर {operation} ऑपरेशन लागू नहीं करता है।',
  'server.endpointNotImplemented': '{endpoint} एंडपॉइंट अभी तक लागू नहीं किया गया है।',
  'server.permissionDenied':
    '{operation} {resource} संसाधन के लिए वर्तमान उपयोगकर्ता को अनुमति नहीं है।',
  'server.entityNotFound': '{class} मानदंड {criteria} के साथ नहीं मिला',
  'server.jsonSyntax': 'JSON: सिंटैक्स त्रुटि',
  'server.jsonNotContainer': 'JSON: सामग्री एक ऑब्जेक्ट या ऐरे होनी चाहिए।',
  'server.invalidContentType':
    'अमान्य Content-Type हेडर। "application/json" की अपेक्षा, प्राप्त हुआ "{received}"।',
  'server.bodyNotObject': 'अनुरोध बॉडी एक JSON ऑब्जेक्ट होनी चाहिए।',
  'server.siteItemsOneSelector': '"query", "items" या "item_sets" में से ठीक एक',
  'server.invalidCredentials': 'ईमेल या पासवर्ड अमान्य है',
  'server.inactiveAccount': 'उपयोगकर्ता सक्रिय नहीं है।',
  'server.unknownKey': 'अमान्य पासवर्ड निर्माण कुंजी।',
  'server.authenticationRequired': 'प्रमाणीकरण आवश्यक है।',
  'server.invalidPassword': 'पासवर्ड अमान्य है',
  'server.crossOriginRefused': 'सत्र-प्रमाणित लेखन कॉन्फ़िगर किए गए बेस URL से आना चाहिए।',
  'server.propertyNotRecognized': 'गुण "{term}" मान्यता प्राप्त नहीं है।',
  'server.resourceNotFound': 'id {id} के साथ संसाधन नहीं मिला।',
  'server.templateRequiresValue':
    '"{template}" संसाधन टेम्पलेट के लिए एक "{property}" मान आवश्यक है',
  'server.itemSetsNotArray': 'आइटम सेट एक ऐरे होना चाहिए',
  'server.mediaNotArray': 'मीडिया एक ऐरे होना चाहिए',
  'server.mediaNotThisItem': 'मीडिया इस आइटम से संबंधित होना चाहिए।',
  'server.primaryMediaNotThisItem': 'प्राथमिक मीडिया इस आइटम से संबंधित होना चाहिए।',
  'server.mediaIngesterRequired': 'मीडिया को एक इंजस्टर (ingester) सेट करना चाहिए।',
  'server.mediaIngesterInvalid': 'मीडिया को एक वैध इंजस्टर सेट करना चाहिए।',
  'server.mediaItemRequired': 'मीडिया को एक आइटम से संबंधित होना चाहिए।',
  'server.noFileUploaded': 'कोई फ़ाइल अपलोड नहीं की गई',
  'server.assetMediaType':
    '"{file}" को सत्यापित करने में त्रुटि। मीडिया प्रकार "{media}" वाली फ़ाइलें संग्रहीत नहीं की जा सकतीं।',
  'server.assetExtension':
    '"{file}" को सत्यापित करने में त्रुटि। रिज़ॉल्व्ड एक्सटेंशन "{extension}" वाली फ़ाइलें संग्रहीत नहीं की जा सकतीं।',
  'server.assetNameRequired': 'एसेट का नाम होना चाहिए।',
  'server.classesNotArray': 'क्लासेज एक ऐरे (array) होनी चाहिए',
  'server.propertiesNotArray': 'प्रॉपर्टीज़ एक ऐरे (array) होनी चाहिए',
  'server.namespaceUriEmpty': 'नेमस्पेस URI खाली नहीं हो सकता।',
  'server.namespaceUriTaken': 'नेमस्पेस URI "{uri}" पहले से लिया जा चुका है।',
  'server.prefixEmpty': 'प्रीफ़िक्स खाली नहीं हो सकता।',
  'server.prefixInvalid': 'प्रीफ़िक्स में केवल अक्षर, संख्याएँ, हाइफ़न और अंडरस्कोर हो सकते हैं।',
  'server.prefixTaken': 'प्रीफ़िक्स "{prefix}" पहले से लिया जा चुका है।',
  'server.prefixReserved': 'प्रीफ़िक्स "{prefix}" आरक्षित है।',
  'server.labelEmpty': 'लेबल खाली नहीं हो सकता।',
  'server.labelTaken': 'लेबल पहले से लिया जा चुका है।',
  'server.localNameEmpty': 'स्थानीय नाम खाली नहीं हो सकता।',
  'server.localNameTaken': 'स्थानीय नाम "{name}" पहले से लिया जा चुका है।',
  'server.vocabularyRequired': 'शब्दावली निर्धारित की जानी चाहिए।',
  'server.duplicateTemplateProperty': '{id} ID के साथ डुप्लिकेट प्रॉपर्टी जोड़ने का प्रयास',
  'server.vocabularyNoFile': 'फ़ाइल आयात रणनीति के लिए कोई फ़ाइल निर्दिष्ट नहीं है।',
  'server.vocabularyNoUrl': 'URL आयात रणनीति के लिए कोई URL निर्दिष्ट नहीं है।',
  'server.vocabularyNotParsed': 'शब्दावली फ़ाइल को पार्स नहीं किया जा सका।',
  'server.vocabularyNotLoaded': 'URL से शब्दावली लोड नहीं की जा सकी।',
  'server.vocabularyNoMembers':
    'कोई क्लास या प्रॉपर्टी नहीं मिली। क्या आप सुनिश्चित हैं कि आपने सही नेमस्पेस URI का उपयोग किया है?',
  'server.vocabularyPermanent': 'एक स्थायी शब्दावली को संपादित नहीं किया जा सकता',
  'server.vocabularyFormatUnknown':
    'फ़ाइल प्रारूप का पता नहीं लगाया जा सका। सूची से प्रारूप चुनें।',
  'server.vocabularyFormatInvalid': '"{format}" शब्दावली फ़ाइल प्रारूप नहीं है।',
  'server.vocabularyUrlScheme': 'केवल http और https URL आयात किए जा सकते हैं।',
  'server.vocabularyUrlPrivate':
    'URL एक निजी या स्थानीय नेटवर्क पते को इंगित करता है, जिसे सर्वर प्राप्त नहीं करता है।',
  'server.vocabularyUrlTooLarge': 'URL पर फ़ाइल {size} से बड़ी है।',
  'server.vocabularyUrlTimeout': 'URL पर सर्वर ने समय पर उत्तर नहीं दिया।',
  'server.vocabularyUrlStatus': 'URL पर सर्वर ने स्थिति {status} के साथ उत्तर दिया।',
  'server.vocabularyChangeStale':
    '"{name}" की तुलना में परिवर्तन हुआ है। फ़ाइल की फिर से तुलना करें।',
  'server.vocabularyMemberMissing': '"{name}" इस शब्दावली का सदस्य नहीं है।',
  'server.vocabularyCaseClash':
    '"{first}" और "{second}" केवल अक्षर के केस में भिन्न हैं, और यह संग्रह दोनों को संग्रहीत नहीं कर सकता।',
  'server.nameEmpty': 'नाम खाली नहीं हो सकता।',
  'server.emailTaken': 'ईमेल {email} पहले से लिया जा चुका है।',
  'server.userRoleRequired': 'उपयोगकर्ताओं की एक भूमिका होनी चाहिए।',
  'server.invalidEmail':
    'इनपुट एक वैध ईमेल पता नहीं है। बुनियादी प्रारूप local-part@hostname का उपयोग करें',
  'server.siteTitleRequired': 'एक साइट का शीर्षक होना चाहिए।',
  'server.siteThemeRequired': 'एक साइट का थीम होना चाहिए।',
  'server.siteItemPoolRequired': 'एक साइट में आइटम पूल डेटा होना चाहिए।',
  'server.homepageNotInSite': 'एक होमपेज अपनी पैरेंट साइट से संबंधित होना चाहिए।',
  'server.slugEmpty': 'स्लग खाली नहीं हो सकता।',
  'server.slugInvalid': 'एक स्लग में केवल अक्षर, संख्या, अंडरस्कोर और हाइफ़न हो सकते हैं।',
  'server.slugTaken': 'स्लग "{slug}" पहले से ही लिया जा चुका है।',
  'server.pageSiteRequired': 'एक पेज साइट से संबंधित होना चाहिए।',
  'server.pageTitleRequired': 'एक पेज का शीर्षक होना चाहिए।',
  'server.blockLayoutRequired': 'सभी ब्लॉक्स का लेआउट होना चाहिए।',
  'server.blockDataScalar': 'ब्लॉक डेटा स्केलर वैल्यू नहीं होना चाहिए।',
  'server.siteSettingsNotObject': 'अपेक्षित ऑब्जेक्ट या नल, प्राप्त हुआ {kind}।',
  'server.cssTooLarge':
    'कस्टम CSS: लाइन {line}, कॉलम {column}: कस्टम CSS अधिकतम {max} बाइट्स हो सकती है; यह {bytes} है।',
  'server.cssImport': 'कस्टम CSS: लाइन {line}, कॉलम {column}: @import की अनुमति नहीं है।',
  'server.cssDocument': 'कस्टम CSS: लाइन {line}, कॉलम {column}: @document की अनुमति नहीं है।',
  'server.cssForeignUrl':
    'कस्टम CSS: लाइन {line}, कॉलम {column}: url({url}) की अनुमति नहीं है; एक URL /files/ या /assets/ के अंतर्गत एक पथ होना चाहिए।',
  'server.cssExpression': 'कस्टम CSS: लाइन {line}, कॉलम {column}: expression() की अनुमति नहीं है।',
  'server.cssBehavior': 'कस्टम CSS: लाइन {line}, कॉलम {column}: behavior की अनुमति नहीं है।',
  'server.cssMozBinding': 'कस्टम CSS: लाइन {line}, कॉलम {column}: -moz-binding की अनुमति नहीं है।',
  'server.cssStyleBreakout': 'कस्टम CSS: लाइन {line}, कॉलम {column}: </style की अनुमति नहीं है।',
  'server.cssBadUrl': 'कस्टम CSS: लाइन {line}, कॉलम {column}: यह url( वैध CSS नहीं है।',
  'server.cssUnclosedString': 'कस्टम CSS: लाइन {line}, कॉलम {column}: यह स्ट्रिंग बंद नहीं है।',
  'server.cssStrayClose':
    'कस्टम CSS: लाइन {line}, कॉलम {column}: {bracket} कुछ भी बंद नहीं करता है।',
  'server.navNotArray': 'अमान्य नेविगेशन: नेविगेशन एक ऐरे होना चाहिए',
  'server.navLinkMissingType': 'अमान्य नेविगेशन: लिंक में टाइप (type) अनुपस्थित है',
  'server.navLinkMissingData': 'अमान्य नेविगेशन: लिंक में डेटा (data) अनुपस्थित है',
  'server.navInvalidLinkData': 'अमान्य नेविगेशन: अमान्य लिंक डेटा',
  'server.navPageLinksNotUnique': 'अमान्य नेविगेशन: पेज लिंक्स अद्वितीय होने चाहिए',
  'server.navLinksNotArray': 'अमान्य नेविगेशन: लिंक्स एक ऐरे होने चाहिए',
  'server.navPageLinkMissingPage': 'अमान्य नेविगेशन: पेज लिंक में पेज ID अनुपस्थित है',
  'server.navUrlMissingLabel': 'अमान्य नेविगेशन: URL लिंक में लेबल (label) अनुपस्थित है',
  'server.navUrlMissingUrl': 'अमान्य नेविगेशन: URL लिंक में URL अनुपस्थित है',
  'server.navUrlInvalidCharacter': 'अमान्य नेविगेशन: URL लिंक में अमान्य वर्ण है',
  'server.navUrlInvalidScheme': 'अमान्य नेविगेशन: URL लिंक में अमान्य स्कीम है',
  'server.settingsDenied':
    'वर्तमान उपयोगकर्ता के लिए वैश्विक सेटिंग्स तक पहुँचने के लिए अनुमति अस्वीकृत।',
  'server.settingsNotObject': 'एक ऑब्जेक्ट की अपेक्षा थी।',
  'server.settingUnavailable': 'सेटिंग {key} उपलब्ध नहीं है।',
  'server.settingNotText': 'सेटिंग {key} एक स्ट्रिंग या नल होनी चाहिए।',
  'server.settingNotFlag': 'सेटिंग {key} सत्य, असत्य या नल होनी चाहिए।',
  'server.settingNotNames': 'सेटिंग {key} स्ट्रिंग्स की एक सूची या नल होनी चाहिए।',
};
