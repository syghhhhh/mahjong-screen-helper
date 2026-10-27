# 完整笔画输入 fixture

这些原创几何图不含游戏截图或字体素材。手工指定预期矩形及像素数，
由独立 Pillow NEAREST 产生 128×64 的单字节二值参考，不调用 Rust 定位实现。
覆盖连续扩展、四边补全、装饰排除及仅外接框相交而笔画不相交的情况。

`generate.py` 只供开发者重建 fixture；CI 和便携程序都不需要 Python。
旧 `action-input` fixture 和 v1 输入契约继续保留。
