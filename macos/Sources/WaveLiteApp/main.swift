import AppKit

let application = NSApplication.shared
let delegate = AppDelegate()
application.delegate = delegate
application.setActivationPolicy(.regular)

let menu = NSMenu()
let applicationMenuItem = NSMenuItem()
menu.addItem(applicationMenuItem)
let applicationMenu = NSMenu()
applicationMenu.addItem(withTitle: "Quit WaveLite", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
applicationMenuItem.submenu = applicationMenu
application.mainMenu = menu

application.run()
