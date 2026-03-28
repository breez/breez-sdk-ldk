import UserNotifications
import Foundation

struct ReceivePaymentNotificationRequest: Codable {
    let payment_hash: String
}

class ReceivePaymentTask : TaskProtocol {
    fileprivate let TAG = "ReceivePaymentTask"
    
    internal var payload: String
    internal var contentHandler: ((UNNotificationContent) -> Void)?
    internal var bestAttemptContent: UNMutableNotificationContent?
    internal var logger: ServiceLogger
    internal var receivedPayment: Payment? = nil
    internal var breezSDK: BlockingBreezServices? = nil
    
    init(payload: String, logger: ServiceLogger, contentHandler: ((UNNotificationContent) -> Void)? = nil, bestAttemptContent: UNMutableNotificationContent? = nil) {
        self.payload = payload
        self.contentHandler = contentHandler
        self.bestAttemptContent = bestAttemptContent
        self.logger = logger
    }
    
    public func onEvent(e: BreezEvent) {
        switch e {
        case .invoicePaid(details: let details):
            self.logger.log(tag: TAG, line: "Received payment.\nPayment Hash:\(details.paymentHash)", level: "INFO")
            let loadedPayment: Payment?
            do {
                loadedPayment = try self.breezSDK?.paymentByHash(hash: details.paymentHash)
            } catch let e {
                self.logger.log(tag: TAG, line: "Failed to load payment by hash \(details.paymentHash): \(e)", level: "WARN")
                loadedPayment = nil
            }
            receivedPayment = loadedPayment
            break
        case .synced:
            self.logger.log(tag: TAG, line: "got synced event", level: "INFO")
            if self.receivedPayment != nil {
                self.onShutdown()
            }
            break
        default:
            break
        }
    }
    
    func start(breezSDK: BlockingBreezServices) throws {
        do {
            self.breezSDK = breezSDK
            let request = try JSONDecoder().decode(ReceivePaymentNotificationRequest.self, from: self.payload.data(using: .utf8)!)
            let existingPayment = try breezSDK.paymentByHash(hash: request.payment_hash)
            if existingPayment != nil {
                self.receivedPayment = existingPayment
                self.logger.log(tag: TAG, line: "Found payment for hash \(request.payment_hash)", level: "INFO")
                self.onShutdown()
            }
        } catch let e {
            self.logger.log(tag: TAG, line: "Failed to call start of receive payment notification: \(e)", level: "ERROR")
        }
    }
    
    func onShutdown() {
        let successReceivedPayment = ResourceHelper.shared.getString(key: Constants.PAYMENT_RECEIVED_NOTIFICATION_TITLE, validateContains: "%d", fallback: Constants.DEFAULT_PAYMENT_RECEIVED_NOTIFICATION_TITLE)
        let failReceivedPayment = ResourceHelper.shared.getString(key: Constants.PAYMENT_RECEIVED_NOTIFICATION_FAILURE_TITLE, fallback: Constants.DEFAULT_PAYMENT_RECEIVED_NOTIFICATION_FAILURE_TITLE)
        let title = self.receivedPayment != nil ? String(format: successReceivedPayment, self.receivedPayment!.amountMsat/1000) : failReceivedPayment
        self.displayPushNotification(title: title, logger: self.logger, threadIdentifier: Constants.NOTIFICATION_THREAD_PAYMENT_RECEIVED)
    }
}
