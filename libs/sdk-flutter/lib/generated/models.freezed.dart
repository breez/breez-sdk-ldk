// dart format width=80
// coverage:ignore-file
// GENERATED CODE - DO NOT MODIFY BY HAND
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'models.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;
/// @nodoc
mixin _$PaymentDetails {

 Object get data;



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is PaymentDetails&&const DeepCollectionEquality().equals(other.data, data));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(data));

@override
String toString() {
  return 'PaymentDetails(data: $data)';
}


}

/// @nodoc
class $PaymentDetailsCopyWith<$Res>  {
$PaymentDetailsCopyWith(PaymentDetails _, $Res Function(PaymentDetails) __);
}


/// @nodoc


class PaymentDetails_Ln extends PaymentDetails {
  const PaymentDetails_Ln({required this.data}): super._();
  

@override final  LnPaymentDetails data;

/// Create a copy of PaymentDetails
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$PaymentDetails_LnCopyWith<PaymentDetails_Ln> get copyWith => _$PaymentDetails_LnCopyWithImpl<PaymentDetails_Ln>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is PaymentDetails_Ln&&(identical(other.data, data) || other.data == data));
}


@override
int get hashCode => Object.hash(runtimeType,data);

@override
String toString() {
  return 'PaymentDetails.ln(data: $data)';
}


}

/// @nodoc
abstract mixin class $PaymentDetails_LnCopyWith<$Res> implements $PaymentDetailsCopyWith<$Res> {
  factory $PaymentDetails_LnCopyWith(PaymentDetails_Ln value, $Res Function(PaymentDetails_Ln) _then) = _$PaymentDetails_LnCopyWithImpl;
@useResult
$Res call({
 LnPaymentDetails data
});




}
/// @nodoc
class _$PaymentDetails_LnCopyWithImpl<$Res>
    implements $PaymentDetails_LnCopyWith<$Res> {
  _$PaymentDetails_LnCopyWithImpl(this._self, this._then);

  final PaymentDetails_Ln _self;
  final $Res Function(PaymentDetails_Ln) _then;

/// Create a copy of PaymentDetails
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? data = null,}) {
  return _then(PaymentDetails_Ln(
data: null == data ? _self.data : data // ignore: cast_nullable_to_non_nullable
as LnPaymentDetails,
  ));
}


}

/// @nodoc


class PaymentDetails_ClosedChannel extends PaymentDetails {
  const PaymentDetails_ClosedChannel({required this.data}): super._();
  

@override final  ClosedChannelPaymentDetails data;

/// Create a copy of PaymentDetails
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$PaymentDetails_ClosedChannelCopyWith<PaymentDetails_ClosedChannel> get copyWith => _$PaymentDetails_ClosedChannelCopyWithImpl<PaymentDetails_ClosedChannel>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is PaymentDetails_ClosedChannel&&(identical(other.data, data) || other.data == data));
}


@override
int get hashCode => Object.hash(runtimeType,data);

@override
String toString() {
  return 'PaymentDetails.closedChannel(data: $data)';
}


}

/// @nodoc
abstract mixin class $PaymentDetails_ClosedChannelCopyWith<$Res> implements $PaymentDetailsCopyWith<$Res> {
  factory $PaymentDetails_ClosedChannelCopyWith(PaymentDetails_ClosedChannel value, $Res Function(PaymentDetails_ClosedChannel) _then) = _$PaymentDetails_ClosedChannelCopyWithImpl;
@useResult
$Res call({
 ClosedChannelPaymentDetails data
});




}
/// @nodoc
class _$PaymentDetails_ClosedChannelCopyWithImpl<$Res>
    implements $PaymentDetails_ClosedChannelCopyWith<$Res> {
  _$PaymentDetails_ClosedChannelCopyWithImpl(this._self, this._then);

  final PaymentDetails_ClosedChannel _self;
  final $Res Function(PaymentDetails_ClosedChannel) _then;

/// Create a copy of PaymentDetails
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? data = null,}) {
  return _then(PaymentDetails_ClosedChannel(
data: null == data ? _self.data : data // ignore: cast_nullable_to_non_nullable
as ClosedChannelPaymentDetails,
  ));
}


}

/// @nodoc
mixin _$ReportIssueRequest {

 ReportPaymentFailureDetails get data;
/// Create a copy of ReportIssueRequest
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReportIssueRequestCopyWith<ReportIssueRequest> get copyWith => _$ReportIssueRequestCopyWithImpl<ReportIssueRequest>(this as ReportIssueRequest, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ReportIssueRequest&&(identical(other.data, data) || other.data == data));
}


@override
int get hashCode => Object.hash(runtimeType,data);

@override
String toString() {
  return 'ReportIssueRequest(data: $data)';
}


}

/// @nodoc
abstract mixin class $ReportIssueRequestCopyWith<$Res>  {
  factory $ReportIssueRequestCopyWith(ReportIssueRequest value, $Res Function(ReportIssueRequest) _then) = _$ReportIssueRequestCopyWithImpl;
@useResult
$Res call({
 ReportPaymentFailureDetails data
});




}
/// @nodoc
class _$ReportIssueRequestCopyWithImpl<$Res>
    implements $ReportIssueRequestCopyWith<$Res> {
  _$ReportIssueRequestCopyWithImpl(this._self, this._then);

  final ReportIssueRequest _self;
  final $Res Function(ReportIssueRequest) _then;

/// Create a copy of ReportIssueRequest
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') @override $Res call({Object? data = null,}) {
  return _then(_self.copyWith(
data: null == data ? _self.data : data // ignore: cast_nullable_to_non_nullable
as ReportPaymentFailureDetails,
  ));
}

}


/// @nodoc


class ReportIssueRequest_PaymentFailure extends ReportIssueRequest {
  const ReportIssueRequest_PaymentFailure({required this.data}): super._();
  

@override final  ReportPaymentFailureDetails data;

/// Create a copy of ReportIssueRequest
/// with the given fields replaced by the non-null parameter values.
@override @JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReportIssueRequest_PaymentFailureCopyWith<ReportIssueRequest_PaymentFailure> get copyWith => _$ReportIssueRequest_PaymentFailureCopyWithImpl<ReportIssueRequest_PaymentFailure>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ReportIssueRequest_PaymentFailure&&(identical(other.data, data) || other.data == data));
}


@override
int get hashCode => Object.hash(runtimeType,data);

@override
String toString() {
  return 'ReportIssueRequest.paymentFailure(data: $data)';
}


}

/// @nodoc
abstract mixin class $ReportIssueRequest_PaymentFailureCopyWith<$Res> implements $ReportIssueRequestCopyWith<$Res> {
  factory $ReportIssueRequest_PaymentFailureCopyWith(ReportIssueRequest_PaymentFailure value, $Res Function(ReportIssueRequest_PaymentFailure) _then) = _$ReportIssueRequest_PaymentFailureCopyWithImpl;
@override @useResult
$Res call({
 ReportPaymentFailureDetails data
});




}
/// @nodoc
class _$ReportIssueRequest_PaymentFailureCopyWithImpl<$Res>
    implements $ReportIssueRequest_PaymentFailureCopyWith<$Res> {
  _$ReportIssueRequest_PaymentFailureCopyWithImpl(this._self, this._then);

  final ReportIssueRequest_PaymentFailure _self;
  final $Res Function(ReportIssueRequest_PaymentFailure) _then;

/// Create a copy of ReportIssueRequest
/// with the given fields replaced by the non-null parameter values.
@override @pragma('vm:prefer-inline') $Res call({Object? data = null,}) {
  return _then(ReportIssueRequest_PaymentFailure(
data: null == data ? _self.data : data // ignore: cast_nullable_to_non_nullable
as ReportPaymentFailureDetails,
  ));
}


}

// dart format on
