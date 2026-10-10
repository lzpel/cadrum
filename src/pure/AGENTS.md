# 方針

- 各ツール　src/pure/DataExchange/TKDE　などをrustで移植していく　ただしcadrumから参照されているクラスや関数だけでよい
- panicを使ってはいけない、使いそうならresultを返すように設計する
- ビット単位で同等のSTEPを出力することがターゲット
- occtのグローバル状態は引数化して排除
- Rc/Arc と RefCell/Mutex は使わない
- ファイル配置はsrc/pure/<Module>/<Toolkit>/の下にocctパッケージ名のまま置く

## express方針：STEPエンティティは EXPRESS から生成する

TKXCAF と ApplicationFramework（TKBin, TKLCAF, TKCAF, TKCDF）は移植しない。
色付きSTEPは XCAF 文書を経由せず、EXPRESS から生成したエンティティ（STYLED_ITEM から COLOUR_RGB まで）を形状と色の対応表から直接組み立てて書く。
TKXCAF は TKLCAF・TKCAF・TKCDF に依存するので、STEPCAFControl の色の経路を置き換えれば両方まとめて不要になる。
出力は STEPCAFControl_Writer とビット単位で一致させるため、エンティティの順序（ソリッドの後に面）と番号の振り方は同 Writer の手順を再現する。
読み込みも STEPCAFControl_Reader が行う STYLED_ITEM から形状への逆引きだけを再現する。
