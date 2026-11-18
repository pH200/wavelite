import AppKit

let mainWindowContentWidth: CGFloat = 480
let waveFxWindowContentWidth: CGFloat = 1000
let windowMargin: CGFloat = 20
let settingsRowPadding: CGFloat = 8

func valueRow(_ title: String, control: NSControl) -> NSView {
    let row = NSStackView(views: [NSTextField(labelWithString: title), control])
    row.orientation = .horizontal
    row.alignment = .centerY
    row.distribution = .equalCentering
    row.edgeInsets = NSEdgeInsets(
        top: settingsRowPadding,
        left: 12,
        bottom: settingsRowPadding,
        right: 12
    )
    return row
}

func makeGroup(title: String, rows: [NSView]) -> NSStackView {
    let group = NSStackView()
    configureGroup(group, title: title, rows: rows)
    return group
}

func configureGroup(_ group: NSStackView, title: String, rows: [NSView]) {
    group.orientation = .vertical
    group.alignment = .leading
    group.spacing = 6

    let heading = NSTextField(labelWithString: title)
    heading.font = .boldSystemFont(ofSize: 14)
    group.addArrangedSubview(heading)

    let list = makeSettingList(rows: rows)
    group.addArrangedSubview(list)
    heading.widthAnchor.constraint(equalTo: group.widthAnchor).isActive = true
    list.widthAnchor.constraint(equalTo: group.widthAnchor).isActive = true
}

func makeSettingList(rows: [NSView]) -> NSView {
    let list = NSStackView()
    list.orientation = .vertical
    list.alignment = .leading
    list.spacing = 0
    for (index, row) in rows.enumerated() {
        if index > 0 {
            let separator = NSBox()
            separator.boxType = .separator
            list.addArrangedSubview(separator)
            separator.widthAnchor.constraint(equalTo: list.widthAnchor).isActive = true
            separator.heightAnchor.constraint(equalToConstant: 1).isActive = true
        }
        list.addArrangedSubview(row)
        row.widthAnchor.constraint(equalTo: list.widthAnchor).isActive = true
    }

    let box = NSBox()
    box.boxType = .primary
    box.titlePosition = .noTitle
    box.contentViewMargins = .zero
    let content = NSView()
    box.contentView = content
    list.translatesAutoresizingMaskIntoConstraints = false
    content.addSubview(list)
    NSLayoutConstraint.activate([
        list.leadingAnchor.constraint(equalTo: content.leadingAnchor),
        list.trailingAnchor.constraint(equalTo: content.trailingAnchor),
        list.topAnchor.constraint(equalTo: content.topAnchor),
        list.bottomAnchor.constraint(equalTo: content.bottomAnchor),
    ])
    return box
}
