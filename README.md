# parallel-minecraft-asset-downloader

## このツールは何？

Minecraft のアセットを並列にダウンロードします。

## このツールを使う利点

* Minecraft のアセットを並列にダウンロードできる

## インストール

1. [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html) をインストール
2. `cargo install --git https://github.com/KisaragiEffective/parallel-minecraft-downloader/` を実行

## 使い方

コマンドラインから、以下のコマンドを実行:

```sh
parallel-minecraft-downloader -d ~/.minecraft --version 1.12.2
```

* `-d`: `.minecraft` の場所 (参考: [minecraft.wikiの記事](https://ja.minecraft.wiki/w/.minecraft))
* `--version`: Minecraft のバージョン

## ライセンス

[Apache-2.0](https://github.com/KisaragiEffective/parallel-minecraft-downloader/blob/3a0c9c9fe43b4b17f82b35135ee74a751af8f82d/LICENSE)
