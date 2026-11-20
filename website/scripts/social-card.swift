import AppKit

// 一次性生成与官网排版一致的分享图；构建直接使用已保存的PNG，不要求部署环境有Swift。
let destination = CommandLine.arguments[1]
try FileManager.default.createDirectory(atPath: destination, withIntermediateDirectories: true)
let width = 1200
let height = 630
func color(_ hex: UInt32) -> NSColor {
    NSColor(srgbRed: CGFloat((hex >> 16) & 255) / 255,
            green: CGFloat((hex >> 8) & 255) / 255,
            blue: CGFloat(hex & 255) / 255, alpha: 1)
}
for language in ["zh", "en"] {
    let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: width, pixelsHigh: height,
                                  bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true,
                                  isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
    color(0xf8f9f5).setFill()
    NSBezierPath(rect: NSRect(x: 0, y: 0, width: width, height: height)).fill()
    func text(_ value: String, x: CGFloat, top: CGFloat, size: CGFloat, ink: UInt32 = 0x273c31, serif: Bool = false, bold: Bool = false) {
        let font = serif ? NSFont(name: "Songti SC", size: size)! : NSFont.systemFont(ofSize: size, weight: bold ? .semibold : .regular)
        (value as NSString).draw(at: NSPoint(x: x, y: CGFloat(height) - top - size * 1.25),
                                withAttributes: [.font: font, .foregroundColor: color(ink)])
    }
    color(0x365544).setFill()
    NSBezierPath(roundedRect: NSRect(x: 86, y: 510, width: 58, height: 58), xRadius: 16, yRadius: 16).fill()
    let mark = NSBezierPath()
    for points in [[NSPoint(x: 100, y: 551), NSPoint(x: 108, y: 526), NSPoint(x: 116, y: 551)],
                   [NSPoint(x: 114, y: 551), NSPoint(x: 122, y: 526), NSPoint(x: 130, y: 551)]] {
        mark.move(to: points[0]); mark.line(to: points[1]); mark.line(to: points[2])
    }
    color(0xfffdf7).setStroke(); mark.lineWidth = 4; mark.lineCapStyle = .round; mark.stroke()
    text("WorkTwill.", x: 158, top: 69, size: 34, bold: true)
    text(language == "en" ? "YOUR LOCAL WORKSPACE" : "一个安静、专注的个人工作台", x: 88, top: 161, size: 17, ink: 0x68795d)
    text(language == "en" ? "A place for your files." : "让资料就位，", x: 84, top: 215, size: language == "en" ? 66 : 74)
    text(language == "en" ? "Room to keep going." : "让工作继续。", x: 84, top: 315, size: language == "en" ? 68 : 76, ink: 0x68805b, serif: language == "zh")
    color(0xdbe3ce).setStroke()
    let rule = NSBezierPath(); rule.move(to: NSPoint(x: 88, y: 141)); rule.line(to: NSPoint(x: 1112, y: 141)); rule.lineWidth = 1; rule.stroke()
    text(language == "en" ? "Local first  /  No account  /  Originals stay in place" : "本地优先  /  无需登录  /  原文件留在原处", x: 88, top: 514, size: 19, ink: 0x647556)
    text("worktwill.haoclub.com", x: 88, top: 562, size: 15, ink: 0x647556)
    NSGraphicsContext.restoreGraphicsState()
    try bitmap.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: "\(destination)/\(language).png"))
}
