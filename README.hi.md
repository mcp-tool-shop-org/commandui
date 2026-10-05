<p align="center">
  <a href="README.ja.md">日本語</a> | <a href="README.zh.md">中文</a> | <a href="README.es.md">Español</a> | <a href="README.fr.md">Français</a> | <a href="README.md">English</a> | <a href="README.it.md">Italiano</a> | <a href="README.pt-BR.md">Português (BR)</a>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/mcp-tool-shop-org/brand/main/logos/commandui/readme.png" width="400" alt="CommandUI" />
</p>

<p align="center">
  <a href="https://github.com/mcp-tool-shop-org/commandui/actions/workflows/ci.yml"><img src="https://github.com/mcp-tool-shop-org/commandui/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
  <a href="https://codecov.io/gh/mcp-tool-shop-org/commandui"><img src="https://codecov.io/gh/mcp-tool-shop-org/commandui/graph/badge.svg" alt="Coverage" /></a>
  <a href="https://github.com/mcp-tool-shop-org/commandui/releases/latest"><img src="https://img.shields.io/github/v/release/mcp-tool-shop-org/commandui?label=Release" alt="Release" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue" alt="MIT License" /></a>
  <a href="https://mcp-tool-shop-org.github.io/commandui/"><img src="https://img.shields.io/badge/Landing_Page-live-blue" alt="Landing Page" /></a>
  <a href="https://mcp-tool-shop-org.github.io/commandui/handbook/"><img src="https://img.shields.io/badge/Handbook-read-blue" alt="Handbook" /></a>
</p>

यह उन लोगों के लिए एक शेल है जिन्हें टर्मिनल से अलग रखा जाता है। CommandUI प्रत्येक परिणाम को सरल शब्दों में समझाता है, आपको सरल शब्दों में किसी कमांड के लिए पूछने देता है, और किसी भी कमांड को तब तक नहीं चलाता जब तक कि आप उसे नहीं देख लेते और उसकी मंजूरी नहीं दे देते।

## यह किसके लिए है

- जो लोग स्क्रीन रीडर का उपयोग करते हैं, या जो माउस का उपयोग नहीं करते हैं
- कम दृष्टि वाले लोग, जिन्हें बड़े पाठ या उच्च-विपरीत थीम की आवश्यकता होती है
- जो लोग टर्मिनल को समझना मुश्किल पाते हैं, जिनमें शुरुआती और संज्ञानात्मक या सीखने की अक्षमता वाले लोग शामिल हैं
- कोई भी व्यक्ति जो किसी कमांड को चलाने से पहले उसे पढ़ना चाहता है

आपको अभी भी एक वास्तविक शेल मिलता है, जिसमें आपका अपना प्रोफ़ाइल और एक से अधिक सत्र होते हैं। किसी कमांड को टाइप करने से वह उसी तरह काम करता है जैसे पहले करता था।

## इंस्टॉल करें

- **माइक्रोसॉफ्ट स्टोर:** [माइक्रोसॉफ्ट स्टोर पर कमांडयूआई](https://apps.microsoft.com/detail/9NTN1GFQJ91M)। इस अपडेट को प्रकाशित किए जाने तक स्टोर में इसका पुराना संस्करण उपलब्ध है।
- **सीधा डाउनलोड:** `.msi` या सेटअप `.exe` को [गिटहब रिलीज़](https://github.com/mcp-tool-shop-org/commandui/releases/latest) से डाउनलोड करें। ये कोड-हस्ताक्षरित नहीं हैं, इसलिए विंडोज आपसे इन्हें स्थापित करने से पहले इसकी पुष्टि करने के लिए कह सकता है।
- **विंगेट:** `winget install mcp-tool-shop.CommandUI`, विंगेट कैटलॉग में v1.0.2 आने तक v1.0.0 स्थापित करता है।

Windows 10 या 11, x64। Ask को `qwen2.5:14b` मॉडल के साथ उसी कंप्यूटर पर [Ollama](https://ollama.com) की आवश्यकता होती है। बाकी सब कुछ इसके बिना काम करता है।

## यह क्या करता है

- **प्रत्येक परिणाम एक वाक्य में।** "समाप्त। 3 पंक्तियों का आउटपुट।" या "काम नहीं किया (निकास कोड 1)। उस कमांड में एक फ़ाइल या फ़ोल्डर मौजूद नहीं है।" विफलता **इसे ठीक करने का तरीका पूछने** और **फिर से चलाने** का विकल्प प्रदान करती है।
- **सरल शब्दों में पूछें।** कार्य का वर्णन करें, और CommandUI एक कमांड बनाता है, उसे समझाता है, और प्रतीक्षा करता है। **रन प्लान** अनुमोदन है, और **अस्वीकार** कुछ भी नहीं चलाता है। जब यह किसी कमांड को नहीं समझा सकता है, तो यह ऐसा कहता है।
- **सावधानीपूर्वक हाँ।** एक कमांड जो फ़ाइलों को हटाता है या उच्च अनुमतियों की आवश्यकता होती है, तब तक प्रतीक्षा करता है जब तक कि आप फ़ोल्डर का नाम टाइप नहीं कर देते।
- **कमांड अभी भी आपके द्वारा टाइप किए गए को चलाता है।** यदि कोई पंक्ति अनुरोध की तरह पढ़ती है, तो CommandUI वाक्य चलाने के बजाय पूछने का विकल्प प्रदान करता है।
- **आप जिन वर्कफ़्लो को बना सकते हैं।** कमांड की एक सूची बनाएं, उसे संपादित करें, चलाएं और हटाएं। किसी भी विलोपन को पूर्ववत किया जा सकता है। इतिहास आपके द्वारा चुने गए कमांड को सहेज सकता है।
- **इतिहास और स्मृति जिसे आप नियंत्रित करते हैं।** देखें कि क्या चला, और पढ़ें या CommandUI द्वारा नोट की गई किसी भी चीज़ को हटाएं।

## कीबोर्ड और स्क्रीन रीडर के लिए बनाया गया

- परिणाम और त्रुटियां एक बार घोषित की जाती हैं, बिना आपके फोकस को बदले।
- **आउटपुट** (Ctrl+Shift+O) प्रत्येक कमांड के आउटपुट को सरल पाठ के रूप में सूचीबद्ध करता है, प्रत्येक कमांड के लिए एक क्षेत्र, बिना किसी टर्मिनल कोड के।
- **F1** कीबोर्ड सहायता खोलता है। **Ctrl+Shift+R** अंतिम परिणाम पर जाता है। **Ctrl+Shift+A** कमांड और Ask के बीच स्विच करता है।
- प्रत्येक संवाद इसके अंदर फोकस रखता है, और Escape इसे बंद कर देता है और फोकस को उस स्थान पर वापस कर देता है जहां आप थे।
- पाठ का आकार सेटिंग्स में 100% से 200% तक बदलता है। टर्मिनल के नीचे के पैनल को छिपाया जा सकता है।
- विंडोज कंट्रास्ट थीम और कम-गति सेटिंग्स का सम्मान किया जाता है।

**अभी तक क्या परीक्षण नहीं किया गया है:** नैरेटर, NVDA और विंडोज कंट्रास्ट थीम का परीक्षण इस बिल्ड पर लोगों द्वारा नहीं किया गया है। वे बाद के अपडेट में आएंगे। तब तक, उपरोक्त सूची को इस रूप में मानें कि ऐप को क्या करने के लिए बनाया गया है, न कि एक परीक्षण किया गया दावा।

## सुरक्षा

CommandUI आपके मशीन पर चलता है। यह इतिहास, योजनाएं, वर्कफ़्लो, स्मृति और सेटिंग्स को स्थानीय रूप से रखता है, और केवल उन शेल कमांड को चलाता है जिन्हें आप अनुमोदित करते हैं। यह कोई टेलीमेट्री नहीं भेजता है। Ask इस कंप्यूटर पर एक मॉडल से बात करता है। यदि वह मॉडल स्थापित नहीं है, चल नहीं रहा है, या डाउनलोड नहीं किया गया है, तो Ask ऐसा कहता है और कोई कमांड नहीं बनाता है।

धमकी मॉडल और भेद्यता की रिपोर्ट कैसे करें, इसके लिए [SECURITY.md](SECURITY.md) देखें।

## यह क्या नहीं है

- यह चैटबॉट नहीं है, और न ही यह अपने आप से किसी बनाए गए कमांड को चलाने वाली चीज़ है
- यह दावा नहीं है कि इस बिल्ड पर स्क्रीन रीडर या कंट्रास्ट थीम का परीक्षण किया गया है (ऊपर देखें)
- यह कंसोल नहीं है। `apps/console` इस रिपॉजिटरी में एक दूसरा फ्रंट एंड है और यह आपके द्वारा इंस्टॉल किए गए ऐप का हिस्सा नहीं है।

## डेवलपर्स के लिए

```bash
pnpm install
pnpm dev          # browser preview; does not run your shell
pnpm test         # all tests
pnpm typecheck

# Rust
cd apps/desktop/src-tauri
cargo test
```

रिलीज़ बिल्ड से स्टोर अपलोड पैक करें:

```powershell
./packaging/build-store-exe.ps1
./packaging/pack-msix.ps1
```

`pack-msix.ps1` `release/CommandUI_<version>_x64.msix` लिखता है। यह मौजूदा स्टोर उत्पाद के पैकेज नाम, प्रकाशक और निष्पादन योग्य को रखता है, और उस संस्करण को अस्वीकार कर देता है जो अंतिम सबमिट किए गए संस्करण से अधिक नहीं है। फ़ाइल बिना हस्ताक्षर वाली है; पार्टनर सेंटर इसे साइन करता है।

```
commandui/
  apps/desktop/                 — the desktop app you install
  apps/console/                 — Rust terminal front end on the same runtime
  crates/runtime-core/          — shell sessions and events
  crates/runtime-persistence/   — local storage
  crates/runtime-planner/       — the local model Ask uses
  packages/                     — shared types, contracts, state, UI
  packaging/msix/               — Store manifest and logos
```

अधिक: [हैंडबुक](https://mcp-tool-shop.github.io/commandui/handbook/) · [डेवलपर सेटअप](docs/product/developer-setup.md) · [ज्ञात सीमाएं](docs/product/known-limitations.md) · [रिलीज़ चेकलिस्ट](docs/product/release-checklist.md)

## स्थिति

v1.0.2 गिटहब रिलीज़ पर उपलब्ध है। माइक्रोसॉफ्ट स्टोर अपडेट प्रमाणीकरण प्रक्रिया में है। नैरेटर, एनवीडीए और विंडोज कंट्रास्ट थीम के साथ परीक्षण बाद के अपडेट में किया जाएगा।

[MCP Tool Shop](https://mcp-tool-shop.github.io/) द्वारा निर्मित।
