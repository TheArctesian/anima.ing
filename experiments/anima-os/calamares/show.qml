import QtQuick 2.15
Rectangle {
    id: page
    color: "#1B1D21"
    Image {
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        width: Math.min(640, parent.width * 0.7)
        height: parent.height
        source: Qt.resolvedUrl("hero.png")
        sourceClipRect: Qt.rect(1100, 180, 1640, 1560)
        fillMode: Image.PreserveAspectFit
        smooth: true
    }
    Rectangle {
        anchors.fill: parent
        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop { position: 0; color: "#1B1D21" }
            GradientStop { position: 0.4; color: "#801B1D21" }
            GradientStop { position: 1; color: "#001B1D21" }
        }
    }
    Column {
        anchors.left: parent.left
        anchors.leftMargin: 28
        anchors.verticalCenter: parent.verticalCenter
        width: Math.min(420, parent.width - 56)
        spacing: page.height < 340 ? 8 : 12
        Text {
            width: parent.width
            text: "Installing Anima OS"
            horizontalAlignment: Text.AlignLeft
            wrapMode: Text.WordWrap
            color: "#E6E7E9"
            font.family: "IBM Plex Sans"
            font.pixelSize: page.height < 340 ? 24 : 32
            font.weight: Font.Light
        }
        Text {
            width: parent.width
            text: "Keep this computer plugged in and switched on."
            horizontalAlignment: Text.AlignLeft
            wrapMode: Text.WordWrap
            color: "#E6E7E9"
            font.family: "IBM Plex Sans"
            font.pixelSize: 14
        }
        Text {
            width: parent.width
            text: "Progress can stay unchanged for several minutes while files are copied and the system is configured. You can open the log for details."
            horizontalAlignment: Text.AlignLeft
            wrapMode: Text.WordWrap
            color: "#A4A8B0"
            font.family: "IBM Plex Sans"
            font.pixelSize: 13
        }
    }
}
