import XCTest

final class SmokeTests: XCTestCase {
    func testPass() { XCTAssertEqual(2 + 2, 4) }
    func testFailure() { XCTAssertEqual(2 + 3, 4) }
}
