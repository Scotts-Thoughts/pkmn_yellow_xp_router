import XCTest

/// Taps through the main flows on the sample routes (tests/test_data).
final class FlowTests: XCTestCase {
    var app: XCUIApplication!

    override func setUp() {
        continueAfterFailure = false
        app = XCUIApplication()
        let seed = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../tests/test_data").standardized.path
        app.launchEnvironment["XPR_SEED"] = seed
        app.launch()
    }

    /// Screenshot into $XPR_SHOTS (a host folder) for looking at failures.
    func snap(_ name: String) {
        let dir = ProcessInfo.processInfo.environment["XPR_SHOTS"] ?? "/tmp"
        try? XCUIScreen.main.screenshot().pngRepresentation.write(to: URL(fileURLWithPath: dir).appendingPathComponent("\(name).png"))
    }

    override func tearDown() {
        snap("end-\(name.replacingOccurrences(of: " ", with: "_").replacingOccurrences(of: "[", with: "").replacingOccurrences(of: "]", with: "").replacingOccurrences(of: "-", with: ""))")
    }

    private func openRoute(_ name: String) {
        let row = app.buttons.containing(.staticText, identifier: name).firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 15), "route \(name) listed")
        row.tap()
        // a route with unsaved changes asks first
        let discard = app.buttons["Open Without Saving"]
        if discard.waitForExistence(timeout: 1) { discard.tap() }
        XCTAssertTrue(app.buttons["Add Event"].waitForExistence(timeout: 10), "editor opened")
    }

    func testEditorEventAndBattle() {
        openRoute("yellow-pinsir-lv10brock")
        let rival = app.staticTexts["Trainer: Rival1 1 (Pallet Town)"]
        XCTAssertTrue(rival.waitForExistence(timeout: 5))
        rival.tap()
        XCTAssertTrue(app.buttons["Battle Summary"].waitForExistence(timeout: 5))
        app.buttons["Battle Summary"].tap()
        XCTAssertTrue(app.staticTexts["Vicegrip"].waitForExistence(timeout: 10), "battle summary shows moves")
        // highlight a move, then add a pre-fight candy
        app.staticTexts["Vicegrip"].tap()
        let plus = app.buttons.matching(identifier: "Add").firstMatch
        if plus.exists { plus.tap() }
        app.buttons["Pre-Event State"].tap()
        XCTAssertTrue(app.staticTexts["Stats"].waitForExistence(timeout: 5))
        // next event
        app.buttons["Next Event"].firstMatch.tap()
        sleep(1)
        app.navigationBars.buttons.element(boundBy: 0).tap()
        XCTAssertTrue(app.buttons["Add Event"].waitForExistence(timeout: 5))
    }

    func testAddEventAndUndo() {
        openRoute("yellow-pinsir-lv10brock")
        app.buttons["Add Event"].tap()
        XCTAssertTrue(app.staticTexts["Rare Candy"].waitForExistence(timeout: 5))
        app.staticTexts["Rare Candy"].tap()
        XCTAssertTrue(app.buttons["Add"].waitForExistence(timeout: 5))
        app.buttons["Add"].tap()
        XCTAssertTrue(app.staticTexts.containing(NSPredicate(format: "label CONTAINS 'Rare Candy'")).firstMatch.waitForExistence(timeout: 5), "candy event added")
        app.buttons["Undo"].firstMatch.tap()
        // a trainer through the picker
        app.buttons["Add Event"].tap()
        app.staticTexts["Fight Trainer"].tap()
        XCTAssertTrue(app.buttons["Choose a trainer"].waitForExistence(timeout: 5))
        app.buttons["Choose a trainer"].tap()
        let first = app.staticTexts.matching(NSPredicate(format: "label CONTAINS ' Pokémon'")).firstMatch
        XCTAssertTrue(first.waitForExistence(timeout: 5))
        first.tap()
        app.buttons["Add"].tap()
        sleep(1)
    }

    func testMapAndCompare() {
        openRoute("yellow-pinsir-lv10brock")
        app.buttons["More"].firstMatch.tap()
        XCTAssertTrue(app.buttons["Map"].waitForExistence(timeout: 5))
        app.buttons["Map"].tap()
        XCTAssertTrue(app.navigationBars["Kanto"].waitForExistence(timeout: 10))
        let coord = app.windows.firstMatch.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
        coord.tap()
        sleep(2)
        if app.buttons["Done"].exists { app.buttons["Done"].tap() }
        app.navigationBars.buttons.element(boundBy: 0).tap()
        app.buttons["More"].firstMatch.tap()
        app.buttons["Run Summary"].tap()
        XCTAssertTrue(app.navigationBars["Run Summary"].waitForExistence(timeout: 5))
    }

    func testNewRoute() {
        XCTAssertTrue(app.buttons["New Route"].waitForExistence(timeout: 15))
        app.buttons["New Route"].tap()
        let choose = app.staticTexts["Choose a Pokémon"]
        XCTAssertTrue(choose.waitForExistence(timeout: 5))
        sleep(1)
        choose.tap()
        let search = app.searchFields.firstMatch
        XCTAssertTrue(search.waitForExistence(timeout: 5))
        search.tap()
        search.typeText("Pikach")
        let pika = app.staticTexts["Pikachu"]
        XCTAssertTrue(pika.waitForExistence(timeout: 5))
        pika.tap()
        app.buttons["Create"].tap()
        XCTAssertTrue(app.buttons["Add Event"].waitForExistence(timeout: 10), "editor for the new route")
    }

    /// Downloads ~95 MB; run on its own: XPR_ONLY=testImageryDownload scripts/ui_test.sh
    func testImageryDownload() throws {
        try XCTSkipIf(ProcessInfo.processInfo.environment["XPR_NET"]?.isEmpty ?? true, "network test")
        openRoute("platinum_chimchar")
        app.buttons["More"].firstMatch.tap()
        app.buttons["Map"].tap()
        let dl = app.buttons["Download Pictures"]
        XCTAssertTrue(dl.waitForExistence(timeout: 15))
        dl.tap()
        XCTAssertTrue(app.staticTexts["Map pictures installed"].waitForExistence(timeout: 400))
        sleep(4)
        snap("platinum-map")
    }

    /// Screenshots of the deeper screens (XPR_TOUR=1).
    func testTour() throws {
        try XCTSkipIf(ProcessInfo.processInfo.environment["XPR_TOUR"]?.isEmpty ?? true, "tour")
        openRoute("c-porygon-1-")
        snap("t01-editor")
        app.buttons["Filter"].firstMatch.tap(); sleep(1); snap("t02-filters")
        app.buttons["Clear"].firstMatch.tap()
        sleep(1)
        let trainer = app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH 'Trainer:'")).element(boundBy: 0)
        trainer.tap(); sleep(2)
        snap("t03-battle")
        app.swipeUp(); sleep(1); snap("t04-battle-scrolled")
        app.buttons["Pre-Event State"].tap(); sleep(1)
        app.swipeUp(); sleep(1); snap("t05-state-scrolled")
        app.swipeUp(); app.swipeUp(); sleep(1); snap("t06-editor-form")
        app.navigationBars.buttons.element(boundBy: 0).tap(); sleep(1)
        app.buttons["Add Event"].tap(); sleep(1)
        app.staticTexts["Buy Item"].tap(); sleep(1); snap("t07-buy-item")
        app.navigationBars.buttons.element(boundBy: 0).tap()
        app.staticTexts["TM / HM Move"].tap(); sleep(1); snap("t08-tm")
        app.navigationBars.buttons.element(boundBy: 0).tap()
        app.buttons["Cancel"].tap(); sleep(1)
        app.buttons["More"].firstMatch.tap(); sleep(1); snap("t09-more-menu")
        app.buttons["Custom DVs / IVs"].tap(); sleep(2); snap("t10-dvs")
        app.buttons["Cancel"].tap(); sleep(1)
        app.buttons["More"].firstMatch.tap()
        app.buttons["Map"].tap(); sleep(3)
        app.windows.firstMatch.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.55)).doubleTap(); sleep(1)
        app.windows.firstMatch.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.55)).doubleTap(); sleep(2)
        snap("t11-map-zoomed")
        app.windows.firstMatch.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.55)).tap(); sleep(2)
        snap("t12-map-card")
        if app.buttons["Done"].exists { app.buttons["Done"].tap() }
        app.navigationBars.buttons.element(boundBy: 0).tap(); sleep(1)
        app.navigationBars.buttons.element(boundBy: 0).tap(); sleep(1)
        app.buttons["Settings"].tap(); sleep(1); snap("t13-settings")
    }
}
